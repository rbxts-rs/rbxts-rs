// import_named in an instance field initializer, which roblox-ts moves into the constructor, where a constructor
// parameter (a parameter property and a destructured one too) shadows the reused import's name: TypeScript resolves
// the initializer's name to the import, but stock's reused name reads the parameter there. rbxts-rs imports it again
// as `u_1`. A deliberate difference (a stock bug fixed): expected/ is rbxts-rs's output, not stock's
import { u } from "./impa";
export class P {
	x = probeImportFrom("./impa", "u");
	constructor(u: number) {
		print(u);
	}
}
export class Q {
	x = probeImportFrom("./impa", "u");
	constructor(private u: number) {}
}
export class R {
	x = () => probeImportFrom("./impa", "u");
	constructor({ u }: { u: number }) {
		print(u);
	}
}
