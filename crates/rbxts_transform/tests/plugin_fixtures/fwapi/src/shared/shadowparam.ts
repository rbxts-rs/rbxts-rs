// import_named where a parameter shadows the reused import's name: rbxts-rs imports it again under a fresh name
// (`u_1`), where stock reuses the name, which reads the parameter there. A deliberate difference (a stock bug fixed):
// expected/ is rbxts-rs's output, not stock's
import { u } from "./impa";
export function f(u: number) {
	return [u, probeImportFrom("./impa", "u")];
}
export const g = (u: string) => [u, probeImportFrom("./impa", "u")];
export function h({ u }: { u: boolean }) {
	return [u, probeImportFrom("./impa", "u")];
}
