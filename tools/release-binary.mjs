#!/usr/bin/env node
// release-binary.mjs: builds the prebuilt rbxtsc of a release from the compiler skeleton (templates/compiler) and
// packs it as the release zips: `rbxts-rs-<version>-<platform>.zip` and the fixed-name `rbxtsc-<platform>.zip`, the
// same bytes. Each holds the binary (the only executable, as Rokit expects), LICENSE, THIRD-PARTY-NOTICES.md and
// README.md, the text files byte for byte as git has them at the release tag (`git archive`). It prints the preset's
// `binaries` entry (url, sha256) and the config hash. It publishes nothing.
//
//   node tools/release-binary.mjs [--out <dir>] [--ref <git ref>] [--allow-dirty] [--allow-patch]
//                                 [--allow-preset-mismatch]
//
//   --out                    where the zips go (default target/release-binary)
//   --ref                    the commit the text files come from (default the tag the skeleton pins, v<version>)
//   --allow-dirty            build a skeleton with uncommitted changes (a test build; its hash is not the committed one)
//   --allow-patch            build a skeleton whose Cargo.toml has a [patch] (a test build, never a release)
//   --allow-preset-mismatch  build although preset.json's "config" is not the skeleton's hash, or its "version" not
//                            the tag rbxts_cli pins (a test build: the preset would not download the binary)
//
// The binary is built from the skeleton package itself, so the preset's config hash (rbxtsc.mjs's configHash over
// Cargo.toml, Cargo.lock, rust-toolchain.toml and src/) is the hash of the files it was built from. The skeleton pins
// rbxts_cli (and any plugin) by git tag, and Cargo.lock records those git sources, so a release binary is built from
// the real pushed tag, never through a [patch] to a local path (a [patch] rewrites the lock and changes the hash).
//
// The release order:
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

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import zlib from "node:zlib";

const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SKELETON = path.join(REPO, "templates", "compiler");
const RELEASES = "https://github.com/rbxts-rs/rbxts-rs/releases/download";
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
 * must be the hash (or the preset would not download the binary) and its "version" the tag rbxts_cli pins.
 */
export function presetProblems(preset, hash, tag) {
	const problems = [];
	if (preset.config !== hash) {
		const where = "`node templates/compiler/rbxtsc.mjs --where` prints it";
		problems.push(`"config" is ${JSON.stringify(preset.config)}: set it to "${hash}" (${where})`);
	}
	if (preset.version !== tag) {
		problems.push(`"version" is ${JSON.stringify(preset.version)}: set it to "${tag}" (the tag rbxts_cli pins in Cargo.toml)`);
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

/**
 * The shared crates whose version or source differ between the skeleton's lock and the workspace's. The rbxts-rs
 * crates are path packages in the workspace and git packages in the skeleton, so they are not compared.
 */
export function lockDifferences(skeletonLock, workspaceLock) {
	const skeleton = lockPackages(skeletonLock);
	const workspace = lockPackages(workspaceLock);
	const differences = [];
	for (const [name, versions] of skeleton) {
		if (name.startsWith("rbxts_")) continue;
		const theirs = workspace.get(name);
		if (!theirs) differences.push(`${name}: ${versions.join(", ")} (not in the workspace lock)`);
		else if (versions.join() !== theirs.join()) differences.push(`${name}: ${versions.join(", ")}, workspace ${theirs.join(", ")}`);
	}
	return differences;
}

function git(args, options = {}) {
	const r = spawnSync("git", ["-C", REPO, ...args], { maxBuffer: 1 << 28, ...options });
	if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${r.stderr?.toString().trim()}`);
	return r.stdout;
}

function parseArgs(argv) {
	const options = {
		out: path.join(REPO, "target", "release-binary"),
		ref: undefined,
		allowDirty: false,
		allowPatch: false,
		allowPresetMismatch: false,
	};
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i];
		if (a === "--out") options.out = path.resolve(argv[++i]);
		else if (a === "--ref") options.ref = argv[++i];
		else if (a === "--allow-dirty") options.allowDirty = true;
		else if (a === "--allow-patch") options.allowPatch = true;
		else if (a === "--allow-preset-mismatch") options.allowPresetMismatch = true;
		else throw new Error(`unknown argument ${a} (usage in the header of tools/release-binary.mjs)`);
	}
	return options;
}

async function main(argv) {
	const options = parseArgs(argv);
	const { configHash, hostTriple } = await import(pathToFileURL(path.join(SKELETON, "rbxtsc.mjs")).href);
	const cargoToml = fs.readFileSync(path.join(SKELETON, "Cargo.toml"), "utf8");
	const preset = JSON.parse(fs.readFileSync(path.join(SKELETON, "preset.json"), "utf8"));
	const tag = /^rbxts_cli\s*=\s*\{[^}]*\btag\s*=\s*"v([^"]+)"/m.exec(cargoToml)?.[1];
	if (!tag) throw new Error("templates/compiler/Cargo.toml: rbxts_cli has no `tag = \"v<version>\"`");
	if (/^\s*\[patch/m.test(cargoToml) && !options.allowPatch) {
		throw new Error("templates/compiler/Cargo.toml has a [patch]: a release binary is built from the pushed tag only");
	}
	const dirty = git(["status", "--porcelain", "--", "templates/compiler"]).toString().trim();
	if (dirty && !options.allowDirty) throw new Error(`templates/compiler has uncommitted changes:\n${dirty}`);
	const ref = options.ref ?? `v${tag}`;
	const commit = git(["rev-parse", "--verify", `${ref}^{commit}`]).toString().trim();
	const differences = lockDifferences(
		fs.readFileSync(path.join(SKELETON, "Cargo.lock"), "utf8"),
		git(["show", `${commit}:Cargo.lock`]).toString(),
	);
	if (differences.length > 0) {
		throw new Error(`templates/compiler/Cargo.lock differs from the workspace lock at ${ref}:\n  ${differences.join("\n  ")}`);
	}
	const triple = hostTriple();
	const platform = PLATFORMS[triple] ?? triple;
	const exeName = process.platform === "win32" ? `${preset.bin}.exe` : preset.bin;
	const hash = configHash(SKELETON);
	const problems = presetProblems(preset, hash, tag);
	if (problems.length > 0 && !options.allowPresetMismatch) {
		throw new Error(
			`templates/compiler/preset.json does not match the skeleton (the release order's step 2):\n  ${problems.join("\n  ")}\n` +
				"  Commit the fix and run this again (--allow-preset-mismatch builds anyway: a test build).",
		);
	}

	const targetDir = path.join(os.tmpdir(), `rbxtsc-release-${hash.slice(0, 12)}`);
	const env = { ...process.env };
	delete env.RUSTUP_TOOLCHAIN;
	delete env.CARGO_TARGET_DIR;
	delete env.CARGO_BUILD_TARGET;
	let exe;
	fs.rmSync(targetDir, { recursive: true, force: true });
	try {
		const args = ["build", "--release", "--locked", "--target-dir", targetDir];
		console.error(`building ${preset.bin} ${tag} (${triple}) from templates/compiler: cargo ${args.join(" ")}`);
		const r = spawnSync("cargo", args, { cwd: SKELETON, env, stdio: ["ignore", "inherit", "inherit"] });
		if (r.error || r.status !== 0) throw new Error(`the build failed${r.error ? ` (${r.error.message})` : ""}`);
		exe = fs.readFileSync(path.join(targetDir, "release", exeName));
	} finally {
		fs.rmSync(targetDir, { recursive: true, force: true });
	}

	const archived = tarFiles(git(["-c", "core.autocrlf=false", "archive", "--format=tar", commit, "--", ...TEXT_FILES]));
	for (const name of TEXT_FILES) {
		const blob = git(["cat-file", "blob", `${commit}:${name}`]);
		if (!archived.has(name) || Buffer.compare(archived.get(name), blob) !== 0) {
			throw new Error(`${name}: git archive's copy is not the blob at ${ref}`);
		}
	}
	const mtime = new Date(Number(git(["log", "-1", "--format=%ct", commit]).toString().trim()) * 1000);
	const zip = makeZip(
		[
			{ name: exeName, data: exe, mode: 0o755 },
			...TEXT_FILES.map((name) => ({ name, data: archived.get(name), mode: 0o644 })),
		],
		mtime,
	);
	const sha256 = createHash("sha256").update(zip).digest("hex");
	const versioned = `rbxts-rs-${tag}-${platform}.zip`;
	const fixed = `${preset.bin}-${platform}.zip`;
	fs.mkdirSync(options.out, { recursive: true });
	fs.writeFileSync(path.join(options.out, versioned), zip);
	fs.writeFileSync(path.join(options.out, fixed), zip);

	const entry = { [triple]: { url: `${RELEASES}/v${tag}/${versioned}`, sha256 } };
	console.log(`${preset.bin} ${tag} (${triple}), text files from ${ref} (${commit.slice(0, 12)})`);
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
