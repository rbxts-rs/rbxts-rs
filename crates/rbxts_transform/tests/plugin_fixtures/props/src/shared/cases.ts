// Every type the probes list, one alias each.
import type { Big, Box, First, Key, PC, UA, mkOne, mkTwo, s0, s1 } from "./decls";
import type { Mid, Second, UB } from "./zz_late";

// bases in two files: own members, then the inherited ones by (file, position). `First`'s members sit, in their own
// file, at offsets inside this declaration, which TypeScript 7 takes for own members (`isDeclarationContainedBy`
// compares no files): `f2, f1, own, s2, s1`
export interface Both extends Second, First {
	own: number;
}
declare const objA: { a2: number; a1: number };
declare const objB: { b1: number; a2: string };
const spread = { ...objA, ...objB, last: 1 };
declare const big: Big;
const { a: _a, ...rest } = big;

export type Cases = {
	chain: PC;
	both: Both;
	literalUnion: { b: 1; a: 2 } | { a: 3; b: 4; c: 5 };
	interfaceUnion: UA | UB;
	interfaceUnionReversed: UB | UA;
	recordOfKeys: Record<"b" | "a" | "c", number>;
	recordOfEnum: Record<Key, number>;
	recordOfNumbers: Record<3 | 1 | 2, string>;
	recordOfTemplate: Record<`k${2 | 1}`, number>;
	pick: Pick<Big, "z" | "a" | "m">;
	mappedKeys: { [K in "q" | "p" | "r"]: K };
	mappedRenamed: { [K in Key as `on${K}`]: () => void };
	partial: Partial<Big>;
	omit: Omit<Big, "a">;
	readonly: Readonly<PC>;
	spread: typeof spread;
	rest: typeof rest;
	intersection: First & Second;
	intersectionReversed: Second & First;
	box: Box<string>;
	boxUnion: Box<"b"> | Box<"a">;
	classes: Mid;
	merged: Merged;
	indexFirst: { [k: string]: number; a: number } | { b: number; a: number };
	symbols: { [s1]: number; [s0]: number; plain: number };
	returned: ReturnType<typeof mkOne> | ReturnType<typeof mkTwo>;
};
