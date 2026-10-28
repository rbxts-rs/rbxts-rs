// import_named reuses a user import whose name the file also writes as a property or a type, which shadows no value,
// as Flamework's TS transformer does: a property access, an object literal key, a shorthand property, a class field,
// a destructured property renamed, a local type alias and a type parameter
import { u } from "./impa";
export const o = { u: 1 };
export const s = { u };
export class C {
	u = 2;
	m() {
		return [this.u, probeImportFrom("./impa", "u")];
	}
}
export function renamed() {
	const { u: z } = o;
	return [z, probeImportFrom("./impa", "u")];
}
export function typed<u>(value: u) {
	type w = u;
	const n: w = value;
	return [n, probeImportFrom("./impa", "u")];
}
export function aliased() {
	type u = number;
	const n: u = 1;
	return [n, probeImportFrom("./impa", "u")];
}
export const y = [o.u, s.u, probeImportFrom("./impa", "u")];
