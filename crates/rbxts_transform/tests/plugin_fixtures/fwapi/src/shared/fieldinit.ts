// import_named in field initializers where no constructor parameter shadows the reused import's name: reused, as
// Flamework's TS transformer does (a local of the constructor's body comes after the initializers roblox-ts moves
// into the constructor, and a static initializer stays outside it)
import { u } from "./impa";
export class S {
	x = probeImportFrom("./impa", "u");
	constructor(v: number) {
		const u = v;
		print(u);
	}
}
export class T {
	y = probeImportFrom("./impa", "u");
}
export class U {
	static z = probeImportFrom("./impa", "u");
	constructor(u: number) {
		print(u);
	}
}
