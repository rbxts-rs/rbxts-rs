// plugin imports, as Flamework's TS transformer adds them: one declaration per path, the paths in the order their
// first import was asked for, each listing its imports in that order. The string literals `"u"` and `"t"` take those
// names (TypeScript 7.1's identifier set), so the locals are `u_1`, `t_1` and `t_2` in print order: `./impa`'s `t` is
// `t_1` though `./impb`'s was asked for first
export const i1 = probeImportFrom("./impa", "u");
export const i2 = probeImportFrom("./impb", "t");
export const i3 = probeImportFrom("./impa", "t");
export function later() {
	return [probeImportFrom("./impb", "t"), probeImportFrom("./impa", "u")];
}
