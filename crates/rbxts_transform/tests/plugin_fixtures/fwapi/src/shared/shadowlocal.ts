// import_named per site: the user import reused where its name means the import, imported again as `u_1` where a
// block-scoped local, a catch clause's variable or a destructured local shadows it, where stock reuses the name, which
// reads the local there. A deliberate difference (a stock bug fixed): expected/ is rbxts-rs's output, not stock's
import { u } from "./impa";
export const top = probeImportFrom("./impa", "u");
export function block(flag: boolean) {
	if (flag) {
		const u = 1;
		return [u, probeImportFrom("./impa", "u")];
	}
	return [u, probeImportFrom("./impa", "u")];
}
export function caught() {
	try {
		error("x");
	} catch (u) {
		return [u, probeImportFrom("./impa", "u")];
	}
	return undefined;
}
export function destructured(o: { u: number }) {
	const { u } = o;
	return [u, probeImportFrom("./impa", "u")];
}
