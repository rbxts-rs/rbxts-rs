#!/usr/bin/env node
// release-binary.mjs: builds the prebuilt binary of a release from a per-project compiler skeleton (by default this
// repo's templates/compiler) and packs it as the release zips: `<name>-<version>-<platform>.zip` and the fixed-name
// `<bin>-<platform>.zip`, the same bytes. Each holds the binary (the preset's `bin`, the only executable, as Rokit
// expects), LICENSE, THIRD-PARTY-NOTICES.md and README.md, the text files byte for byte as git has them at the release
// tag of the release's repo (`git archive`). It prints the preset's `binaries` entry (url, sha256) and the config hash.
// It publishes nothing.
//
//   node tools/release-binary.mjs [--skeleton <dir>] [--repo <path>] [--name <name>] [--releases <url>]
//                                 [--version-crate <crate>] [--out <dir>] [--ref <git ref>]
//                                 [--allow-dirty] [--allow-patch] [--allow-preset-mismatch]
//
//   --skeleton               the compiler skeleton to build (default templates/compiler): a project's compiler/ folder
//                            (Cargo.toml, Cargo.lock, preset.json, rbxtsc.mjs, ...), in a git repo of its own or this one
//   --repo                   the git repo the release is for (default this one): the text files come from it at --ref,
//                            its Cargo.lock at --ref is the lock the skeleton's is checked against, and its `origin`
//                            remote names the release (below)
//   --name                   the versioned zip's name prefix (default the repo name of --repo's `origin`, else its
//                            folder name: rbxts-rs)
//   --releases               the releases download URL (default https://github.com/<owner>/<repo>/releases/download
//                            from --repo's `origin`, a GitHub https or ssh URL); the entry's url is
//                            <releases>/v<version>/<versioned zip>
//   --version-crate          the dependency of the skeleton's Cargo.toml whose `tag = "v<version>"` is the release's
//                            version (default the dependencies whose `git` is --repo's `origin`: rbxts_cli for
//                            rbxts-rs; several must pin one tag)
//   --out                    where the zips go (default target/release-binary)
//   --ref                    the commit of --repo the text files and the lock come from (default v<version>)
//   --allow-dirty            build a skeleton with uncommitted changes (a test build; its hash is not the committed one)
//   --allow-patch            build a skeleton whose Cargo.toml has a [patch] (a test build, never a release)
//   --allow-preset-mismatch  build although preset.json's "config" is not the skeleton's hash, or its "version" not
//                            the tag the version crate pins (a test build: the preset would not download the binary)
//
// The binary is built from the skeleton package itself, so the preset's config hash (rbxtsc.mjs's configHash over
// Cargo.toml, Cargo.lock, rust-toolchain.toml, .cargo/config.toml and src/) is the hash of the files it was built from. The skeleton pins
// rbxts_cli (and any plugin) by git tag, and Cargo.lock records those git sources, so a release binary is built from
// the real pushed tag, never through a [patch] to a local path (a [patch] rewrites the lock and changes the hash). The
// lock check: every crate the skeleton's lock shares with --repo's lock at the ref has the same version and source,
// except the crates that are path packages there (the repo's own crates, git packages in the skeleton).
//
// The release order (rbxts-rs):
//   1. push the tag v<version>;
//   2. point the skeleton at it: in templates/compiler/Cargo.toml set rbxts_cli's `tag = "v<version>"`, refresh the
//      lock (`cargo update -p rbxts_cli` in templates/compiler; this tool checks that every crate it shares with the
//      workspace lock at the tag has the same version and source), set preset.json's "version" to <version> and its
//      "config" to the skeleton's hash (`node templates/compiler/rbxtsc.mjs --where` prints it as `config:` once the
//      other edits are made), empty its "binaries", and commit (this tool refuses a preset that does not match);
//   3. build with this tool;
//   4. create the GitHub release for the tag with the two zips;
//   5. write the url and sha256 this tool printed into preset.json's "binaries", and commit.
// Steps 2 and 5 land after the tag: the skeleton inside the tag's own tree still pins the previous release.
//
// The release order for a plugin repo, flamework-rs (its compiler skeleton is flamework-template's compiler/, with
// the preset's `bin` rbxtsc-fw):
//   1. push flamework-rs's tag v<version>; its Cargo.lock pins the rbxts-rs release it targets (git packages);
//   2. in flamework-template's compiler/: Cargo.toml pins flamework-rs's crate by `tag = "v<version>"` and rbxts_cli
//      by the rbxts-rs tag flamework-rs's lock has, the lock refreshed (`cargo update -p <crate>`; it must agree with
//      flamework-rs's lock at the tag, rbxts-rs's crates included), preset.json's "version" <version> and "config"
//      the hash, "binaries" emptied; commit;
//   3. from an rbxts-rs checkout: `node tools/release-binary.mjs --skeleton <flamework-template>/compiler
//      --repo <flamework-rs>`, which writes flamework-rs-<version>-<platform>.zip and rbxtsc-fw-<platform>.zip;
//   4. create flamework-rs's GitHub release for the tag with the two zips;
//   5. write the printed url and sha256 into compiler/preset.json's "binaries", and commit.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import zlib from "node:zlib";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SKELETON = path.join(REPO, "templates", "compiler");
const TEXT_FILES = ["LICENSE", "THIRD-PARTY-NOTICES.md", "README.md"];
const PLATFORMS = {
	"x86_64-pc-windows-msvc": "windows-x64",
	"aarch64-pc-windows-msvc": "windows-arm64",
	"x86_64-unknown-linux-gnu": "linux-x64",
	"aarch64-unknown-linux-gnu": "linux-arm64",
	"x86_64-apple-darwin": "macos-x64",
	"aarch64-apple-darwin": "macos-arm64",
};

const CRC_TABLE = (() => {
	const table = new Uint32Array(256);
	for (let n = 0; n < 256; n++) {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		table[n] = c >>> 0;
	}
	return table;
})();

export function crc32(bytes) {
	let c = 0xffffffff;
	for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
	return (c ^ 0xffffffff) >>> 0;
}

/**
 * A zip archive of `files` ({ name, data, mode, store }), deterministic: the entries in the given order, all with
 * `mtime`, deflated unless `store`, Unix modes recorded.
 */
export function makeZip(files, mtime = new Date(Date.UTC(1980, 0, 1))) {
	const time = (mtime.getUTCHours() << 11) | (mtime.getUTCMinutes() << 5) | (mtime.getUTCSeconds() >> 1);
	const date = ((mtime.getUTCFullYear() - 1980) << 9) | ((mtime.getUTCMonth() + 1) << 5) | mtime.getUTCDate();
	const locals = [];
	const centrals = [];
	let offset = 0;
	for (const file of files) {
		const name = Buffer.from(file.name, "utf8");
		const method = file.store ? 0 : 8;
		const body = file.store ? file.data : zlib.deflateRawSync(file.data, { level: 9 });
		const crc = crc32(file.data);
		const local = Buffer.alloc(30);
		local.writeUInt32LE(0x04034b50, 0);
		local.writeUInt16LE(20, 4);
		local.writeUInt16LE(0x0800, 6);
		local.writeUInt16LE(method, 8);
		local.writeUInt16LE(time, 10);
		local.writeUInt16LE(date, 12);
		local.writeUInt32LE(crc, 14);
		local.writeUInt32LE(body.length, 18);
		local.writeUInt32LE(file.data.length, 22);
		local.writeUInt16LE(name.length, 26);
		local.writeUInt16LE(0, 28);
		const central = Buffer.alloc(46);
		central.writeUInt32LE(0x02014b50, 0);
		central.writeUInt16LE((3 << 8) | 20, 4);
		central.writeUInt16LE(20, 6);
		central.writeUInt16LE(0x0800, 8);
		central.writeUInt16LE(method, 10);
		central.writeUInt16LE(time, 12);
		central.writeUInt16LE(date, 14);
		central.writeUInt32LE(crc, 16);
		central.writeUInt32LE(body.length, 20);
		central.writeUInt32LE(file.data.length, 24);
		central.writeUInt16LE(name.length, 28);
		central.writeUInt32LE(((0o100000 | (file.mode ?? 0o644)) << 16) >>> 0, 38);
		central.writeUInt32LE(offset, 42);
		locals.push(local, name, body);
		centrals.push(central, name);
		offset += local.length + name.length + body.length;
	}
	const directory = Buffer.concat(centrals);
	const end = Buffer.alloc(22);
	end.writeUInt32LE(0x06054b50, 0);
	end.writeUInt16LE(files.length, 8);
	end.writeUInt16LE(files.length, 10);
	end.writeUInt32LE(directory.length, 12);
	end.writeUInt32LE(offset, 16);
	return Buffer.concat([...locals, directory, end]);
}

/** The regular files of a tar archive, by path (`git archive --format=tar` output: ustar with pax headers). */
export function tarFiles(tar) {
	const files = new Map();
	let at = 0;
	let paxPath;
	while (at + 512 <= tar.length) {
		const header = tar.subarray(at, at + 512);
		if (header.every((b) => b === 0)) break;
		const field = (start, length) => header.toString("utf8", start, start + length).replace(/\0.*$/s, "");
		const size = parseInt(field(124, 12).trim() || "0", 8);
		const type = String.fromCharCode(header[156] || 0x30);
		const prefix = field(345, 155);
		const name = paxPath ?? (prefix ? `${prefix}/${field(0, 100)}` : field(0, 100));
		const data = tar.subarray(at + 512, at + 512 + size);
		if (type === "x") {
			paxPath = /\d+ path=([^\n]*)\n/.exec(data.toString("utf8"))?.[1];
		} else {
			if (type === "0") files.set(name, Buffer.from(data));
			if (type !== "g") paxPath = undefined;
		}
		at += 512 + Math.ceil(size / 512) * 512;
	}
	return files;
}

/**
 * What is wrong with the skeleton's preset.json for a release of `tag` from a skeleton hashing to `hash`: its "config"
 * must be the hash (or the preset would not download the binary) and its "version" the tag the version crate pins.
 * `skeleton` is the skeleton's folder as the messages name it, `crate` the version crate.
 */
export function presetProblems(preset, hash, tag, { skeleton = "templates/compiler", crate = "rbxts_cli" } = {}) {
	const problems = [];
	if (preset.config !== hash) {
		const where = `\`node ${skeleton}/rbxtsc.mjs --where\` prints it`;
		problems.push(`"config" is ${JSON.stringify(preset.config)}: set it to "${hash}" (${where})`);
	}
	if (preset.version !== tag) {
		problems.push(`"version" is ${JSON.stringify(preset.version)}: set it to "${tag}" (the tag ${crate} pins in Cargo.toml)`);
	}
	return problems;
}

/** `name -> version source` of each package of a Cargo.lock, without path packages. */
export function lockPackages(text) {
	const packages = new Map();
	for (const block of text.split("[[package]]").slice(1)) {
		const name = /^name = "(.*)"$/m.exec(block)?.[1];
		const version = /^version = "(.*)"$/m.exec(block)?.[1];
		const source = /^source = "(.*)"$/m.exec(block)?.[1];
		if (!name || !source) continue;
		const list = packages.get(name) ?? [];
		list.push(`${version} ${source.replace(/#.*$/, "")}`);
		packages.set(name, list.sort());
	}
	return packages;
}

/** The names of a Cargo.lock's path packages (no `source`: the workspace's own crates). */
export function lockPathPackages(text) {
	const names = new Set();
	for (const block of text.split("[[package]]").slice(1)) {
		const name = /^name = "(.*)"$/m.exec(block)?.[1];
		if (name && !/^source = /m.test(block)) names.add(name);
	}
	return names;
}

/**
 * The shared crates whose version or source differ between the skeleton's lock and the release repo's. The repo's own
 * crates are path packages in its lock and git packages in the skeleton, so they are not compared (rbxts-rs's crates
 * for rbxts-rs; flamework-rs's for flamework-rs, whose lock has rbxts-rs's crates as git packages, compared like any
 * other).
 */
export function lockDifferences(skeletonLock, repoLock) {
	const skeleton = lockPackages(skeletonLock);
	const repo = lockPackages(repoLock);
	const own = lockPathPackages(repoLock);
	const differences = [];
	for (const [name, versions] of skeleton) {
		const theirs = repo.get(name);
		if (!theirs && own.has(name)) continue;
		if (!theirs) differences.push(`${name}: ${versions.join(", ")} (not in the repo's lock)`);
		else if (versions.join() !== theirs.join()) differences.push(`${name}: ${versions.join(", ")}, the repo's lock ${theirs.join(", ")}`);
	}
	return differences;
}

/**
 * A GitHub remote URL (`https://github.com/<owner>/<repo>[.git]`, `git@github.com:<owner>/<repo>[.git]`,
 * `ssh://git@github.com/<owner>/<repo>[.git]`) as `{ url: "https://github.com/<owner>/<repo>", owner, name }`, or
 * undefined for anything else.
 */
export function githubRepo(remote) {
	const m = /^(?:https?:\/\/(?:[^@/]+@)?|ssh:\/\/git@|git@)github\.com[/:]([\w.-]+)\/([\w.-]+?)(?:\.git)?\/?$/.exec(remote?.trim() ?? "");
	if (!m) return undefined;
	return { url: `https://github.com/${m[1]}/${m[2]}`, owner: m[1], name: m[2] };
}

/** The releases download URL of a GitHub repo (`githubRepo`'s answer). */
export function releasesUrl(repo) {
	return `${repo.url}/releases/download`;
}

/** The inline-table dependencies of a Cargo.toml that have a `git` source: `{ crate, git, tag, rev }`. */
export function gitDependencies(cargoToml) {
	const deps = [];
	for (const m of cargoToml.matchAll(/^([\w-]+)\s*=\s*\{([^}\n]*)\}/gm)) {
		const field = (key) => new RegExp(`\\b${key}\\s*=\\s*"([^"]*)"`).exec(m[2])?.[1];
		const git = field("git");
		if (git) deps.push({ crate: m[1], git, tag: field("tag"), rev: field("rev") });
	}
	return deps;
}

/**
 * The crate a release's version comes from and that version: `explicit` (--version-crate) if given, else the
 * dependencies whose git URL is the release repo's (`repoUrl`, `githubRepo`'s url), which must all pin one
 * `tag = "v<version>"`.
 */
export function versionCrate(cargoToml, repoUrl, explicit) {
	const deps = gitDependencies(cargoToml);
	let picked;
	if (explicit) {
		picked = deps.filter((d) => d.crate === explicit);
		if (picked.length === 0) throw new Error(`Cargo.toml has no git dependency ${explicit} (--version-crate)`);
	} else {
		const key = (url) => (githubRepo(url)?.url ?? url.replace(/(\.git)?\/*$/, "")).toLowerCase();
		picked = repoUrl ? deps.filter((d) => key(d.git) === key(repoUrl)) : [];
		if (picked.length === 0) {
			const what = repoUrl ?? "the release repo (it has no GitHub origin remote)";
			throw new Error(`Cargo.toml has no git dependency on ${what}: name the crate whose tag is the version with --version-crate`);
		}
	}
	const tags = [...new Set(picked.map((d) => d.tag))];
	const crates = picked.map((d) => d.crate).join(", ");
	if (tags.length > 1) throw new Error(`Cargo.toml: ${crates} pin different tags (${tags.join(", ")})`);
	const version = /^v(.+)$/.exec(tags[0] ?? "")?.[1];
	if (!version) throw new Error(`Cargo.toml: ${crates} has no \`tag = "v<version>"\``);
	return { crate: picked[0].crate, version };
}

/** The release zips' names: the versioned `<name>-<version>-<platform>.zip` and the fixed `<bin>-<platform>.zip`. */
export function releaseNames({ name, version, platform, bin }) {
	return { versioned: `${name}-${version}-${platform}.zip`, fixed: `${bin}-${platform}.zip` };
}

/** A folder as messages name it: relative to the current folder when inside it, else absolute; with `/`. */
function shown(dir) {
	const rel = path.relative(process.cwd(), dir);
	return (rel && !rel.startsWith("..") && !path.isAbsolute(rel) ? rel : dir).replaceAll("\\", "/");
}

function git(cwd, args, options = {}) {
	const r = spawnSync("git", ["-C", cwd, ...args], { maxBuffer: 1 << 28, ...options });
	if (r.status !== 0) throw new Error(`git -C ${shown(cwd)} ${args.join(" ")} failed: ${r.stderr?.toString().trim()}`);
	return r.stdout;
}

export function parseArgs(argv) {
	const options = {
		skeleton: SKELETON,
		repo: REPO,
		name: undefined,
		releases: undefined,
		versionCrate: undefined,
		out: path.join(REPO, "target", "release-binary"),
		ref: undefined,
		allowDirty: false,
		allowPatch: false,
		allowPresetMismatch: false,
	};
	const value = (i, flag) => {
		if (i >= argv.length) throw new Error(`${flag} needs a value`);
		return argv[i];
	};
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		if (a === "--skeleton") options.skeleton = path.resolve(value(++i, a));
		else if (a === "--repo") options.repo = path.resolve(value(++i, a));
		else if (a === "--name") options.name = value(++i, a);
		else if (a === "--releases") options.releases = value(++i, a).replace(/\/+$/, "");
		else if (a === "--version-crate") options.versionCrate = value(++i, a);
		else if (a === "--out") options.out = path.resolve(value(++i, a));
		else if (a === "--ref") options.ref = value(++i, a);
		else if (a === "--allow-dirty") options.allowDirty = true;
		else if (a === "--allow-patch") options.allowPatch = true;
		else if (a === "--allow-preset-mismatch") options.allowPresetMismatch = true;
		else throw new Error(`unknown argument ${a} (usage in the header of tools/release-binary.mjs)`);
	}
	return options;
}

/**
 * Everything about a release short of building it: the checks (no [patch], a committed skeleton, its lock against the
 * repo's at the ref, the preset) and the names. Throws on a failed check unless its --allow flag is set.
 */
export async function prepareRelease(options) {
	const skeleton = options.skeleton;
	const where = shown(skeleton);
	const { configHash, hostTriple } = await import(pathToFileURL(path.join(skeleton, "rbxtsc.mjs")).href);
	const cargoToml = fs.readFileSync(path.join(skeleton, "Cargo.toml"), "utf8");
	const preset = JSON.parse(fs.readFileSync(path.join(skeleton, "preset.json"), "utf8"));
	const remote = spawnSync("git", ["-C", options.repo, "remote", "get-url", "origin"]).stdout?.toString().trim();
	const github = githubRepo(remote);
	let version, crate;
	try {
		({ version, crate } = versionCrate(cargoToml, github?.url, options.versionCrate));
	} catch (e) {
		throw new Error(`${where}/${e.message}`);
	}
	const name = options.name ?? github?.name ?? path.basename(git(options.repo, ["rev-parse", "--show-toplevel"]).toString().trim());
	const releases = options.releases ?? (github && releasesUrl(github));
	if (!releases) throw new Error(`${shown(options.repo)}: its origin remote (${remote || "none"}) is not on GitHub: give --releases <url>`);
	if (/^\s*\[patch/m.test(cargoToml) && !options.allowPatch) {
		throw new Error(`${where}/Cargo.toml has a [patch]: a release binary is built from the pushed tag only`);
	}
	const bins = [...cargoToml.matchAll(/^\[\[bin\]\][^[]*?^name\s*=\s*"([^"]+)"/gms)].map((m) => m[1]);
	if (bins.length > 0 && !bins.includes(preset.bin)) {
		throw new Error(`${where}/preset.json's "bin" is ${JSON.stringify(preset.bin)}, but Cargo.toml builds ${bins.join(", ")}`);
	}
	const dirty = git(skeleton, ["status", "--porcelain", "--", "."]).toString().trim();
	if (dirty && !options.allowDirty) throw new Error(`${where} has uncommitted changes:\n${dirty}`);
	const ref = options.ref ?? `v${version}`;
	const commit = git(options.repo, ["rev-parse", "--verify", `${ref}^{commit}`]).toString().trim();
	const differences = lockDifferences(
		fs.readFileSync(path.join(skeleton, "Cargo.lock"), "utf8"),
		git(options.repo, ["show", `${commit}:Cargo.lock`]).toString(),
	);
	if (differences.length > 0) {
		throw new Error(`${where}/Cargo.lock differs from ${name}'s lock at ${ref}:\n  ${differences.join("\n  ")}`);
	}
	const triple = hostTriple();
	const platform = PLATFORMS[triple] ?? triple;
	const exeName = process.platform === "win32" ? `${preset.bin}.exe` : preset.bin;
	const hash = configHash(skeleton);
	const problems = presetProblems(preset, hash, version, { skeleton: where, crate });
	if (problems.length > 0 && !options.allowPresetMismatch) {
		throw new Error(
			`${where}/preset.json does not match the skeleton (the release order's step 2):\n  ${problems.join("\n  ")}\n` +
				"  Commit the fix and run this again (--allow-preset-mismatch builds anyway: a test build).",
		);
	}
	const { versioned, fixed } = releaseNames({ name, version, platform, bin: preset.bin });
	const url = `${releases}/v${version}/${versioned}`;
	return { skeleton, where, preset, version, crate, name, repo: options.repo, ref, commit, triple, platform, exeName, hash, problems, versioned, fixed, url };
}

/**
 * The release zip: the binary `exe` as `exeName`, then the text files as git has them in `repo` at `commit` (`ref`
 * names it in messages), all dated the commit's time.
 */
export function releaseZip({ repo, commit, ref, exe, exeName }) {
	const archived = tarFiles(git(repo, ["-c", "core.autocrlf=false", "archive", "--format=tar", commit, "--", ...TEXT_FILES]));
	for (const name of TEXT_FILES) {
		const blob = git(repo, ["cat-file", "blob", `${commit}:${name}`]);
		if (!archived.has(name) || Buffer.compare(archived.get(name), blob) !== 0) {
			throw new Error(`${name}: git archive's copy is not the blob at ${ref} of ${shown(repo)}`);
		}
	}
	const mtime = new Date(Number(git(repo, ["log", "-1", "--format=%ct", commit]).toString().trim()) * 1000);
	return makeZip(
		[
			{ name: exeName, data: exe, mode: 0o755 },
			...TEXT_FILES.map((name) => ({ name, data: archived.get(name), mode: 0o644 })),
		],
		mtime,
	);
}

async function main(argv) {
	const options = parseArgs(argv);
	const plan = await prepareRelease(options);
	const { skeleton, where, preset, version: tag, repo, ref, commit, triple, exeName, hash, problems, versioned, fixed, url } = plan;

	const targetDir = path.join(os.tmpdir(), `rbxtsc-release-${hash.slice(0, 12)}`);
	const env = { ...process.env };
	delete env.RUSTUP_TOOLCHAIN;
	delete env.CARGO_TARGET_DIR;
	delete env.CARGO_BUILD_TARGET;
	// rustflags from the environment would replace the skeleton's (.cargo/config.toml: /Brepro)
	for (const name of Object.keys(env)) {
		if (/^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_.*_RUSTFLAGS)$/i.test(name)) delete env[name];
	}
	// Machine paths out of the binary: panic locations name the dependencies' sources under Cargo's home, the standard
	// library's under rustup's (both in the user's folder by default) and the target folder. These flags join the
	// skeleton's for the build's target. The C and C++ dependencies (Luau, built by the cc crate) keep theirs in
	// __FILE__: MSVC's /d1trimfile cuts Cargo's home off.
	const cargoHome = path.resolve(process.env.CARGO_HOME ?? path.join(os.homedir(), ".cargo"));
	const rustupHome = path.resolve(process.env.RUSTUP_HOME ?? path.join(os.homedir(), ".rustup"));
	const remap = [
		`--remap-path-prefix=${cargoHome}=/cargo`,
		`--remap-path-prefix=${rustupHome}=/rustup`,
		`--remap-path-prefix=${targetDir}=/target`,
	];
	if (triple.endsWith("-windows-msvc")) {
		for (const name of ["CFLAGS", "CXXFLAGS"]) env[name] = [env[name], `/d1trimfile:${cargoHome}\\`].filter(Boolean).join(" ");
	}
	const tomlString = (text) => `'${text}'`;
	let exe;
	fs.rmSync(targetDir, { recursive: true, force: true });
	try {
		const args = ["build", "--release", "--locked", "--target-dir", targetDir];
		args.push("--config", `target.${triple}.rustflags = [${remap.map(tomlString).join(", ")}]`);
		console.error(`building ${preset.bin} ${tag} (${triple}) from ${where}: cargo ${args.join(" ")}`);
		const r = spawnSync("cargo", args, { cwd: skeleton, env, stdio: ["ignore", "inherit", "inherit"] });
		if (r.error || r.status !== 0) throw new Error(`the build failed${r.error ? ` (${r.error.message})` : ""}`);
		exe = fs.readFileSync(path.join(targetDir, "release", exeName));
	} finally {
		fs.rmSync(targetDir, { recursive: true, force: true });
	}

	const zip = releaseZip({ repo, commit, ref, exe, exeName });
	const sha256 = createHash("sha256").update(zip).digest("hex");
	fs.mkdirSync(options.out, { recursive: true });
	fs.writeFileSync(path.join(options.out, versioned), zip);
	fs.writeFileSync(path.join(options.out, fixed), zip);

	const entry = { [triple]: { url, sha256 } };
	console.log(`${preset.bin} ${tag} (${triple}), text files from ${plan.name} ${ref} (${commit.slice(0, 12)})`);
	console.log(`  ${path.join(options.out, versioned)}`);
	console.log(`  ${path.join(options.out, fixed)} (the same bytes)`);
	console.log(`  ${zip.length} bytes, sha256 ${sha256}; ${exeName} ${exe.length} bytes`);
	if (options.allowPatch || options.allowDirty || problems.length > 0) {
		console.log("  A TEST BUILD (--allow-patch / --allow-dirty / --allow-preset-mismatch): do not release it.");
	}
	console.log(`config hash: ${hash}`);
	console.log(`preset.json "binaries" entry:`);
	console.log(JSON.stringify(entry, null, "\t"));
}

const invoked = process.argv[1] ? path.resolve(process.argv[1]) : "";
const self = fileURLToPath(import.meta.url);
if (process.platform === "win32" ? invoked.toLowerCase() === self.toLowerCase() : invoked === self) {
	main(process.argv.slice(2)).catch((e) => {
		console.error(`release-binary: ${e.message}`);
		process.exit(1);
	});
}
