// Roblox types through linked packages (tests/fixtures.rs, `junction_roblox_types`): `node_modules`, `@rbxts` or
// each package linked from elsewhere compiles as a copied install. `expected/` is stock roblox-ts's output with a
// real `node_modules`.
import { Workspace } from "@rbxts/services";

function nothing() {}

// possibly-undefined call arguments of Roblox functions keep their parentheses, which truncate a call that returns
// nothing or several values to one value
export function voidArgument() {
	return tonumber(nothing());
}

export function callbackArgument(callback: Callback) {
	return tostring(callback());
}

// Roblox builtins (a library, a Roblox method) stay stable lookups for the evaluation order
export function builtinLibrary(codes: Array<number>) {
	let i = 0;
	const same = (a: string, b: string) => a === b;
	return same(string.char(codes[i++]), string.char(codes[i++]));
}

export function nativeReceiver(values: Array<number>) {
	let part = new Instance("Part");
	part.SetAttribute(
		"value",
		values.map(() => {
			part = new Instance("Part");
			return 7;
		})[0],
	);
	return part;
}

export const gravity = Workspace.Gravity;
