import type { Cases } from "./cases";

declare const c: Cases;
// file 6 of 8: the same probes in another order (reversed, from `spread`), so each checker
// meets the types in another order
export const lists = {
	spread: probeProps(c.spread),
	readonly: probeProps(c.readonly),
	omit: probeProps(c.omit),
	partial: probeProps(c.partial),
	mappedRenamed: probeProps(c.mappedRenamed),
	mappedKeys: probeProps(c.mappedKeys),
	pick: probeProps(c.pick),
	recordOfTemplate: probeProps(c.recordOfTemplate),
	recordOfNumbers: probeProps(c.recordOfNumbers),
	recordOfEnum: probeProps(c.recordOfEnum),
	recordOfKeys: probeProps(c.recordOfKeys),
	interfaceUnionReversed: probeProps(c.interfaceUnionReversed),
	interfaceUnion: probeProps(c.interfaceUnion),
	literalUnion: probeProps(c.literalUnion),
	both: probeProps(c.both),
	chain: probeProps(c.chain),
	returned: probeProps(c.returned),
	symbols: probeProps(c.symbols),
	indexFirst: probeProps(c.indexFirst),
	merged: probeProps(c.merged),
	classes: probeProps(c.classes),
	boxUnion: probeProps(c.boxUnion),
	box: probeProps(c.box),
	intersectionReversed: probeProps(c.intersectionReversed),
	intersection: probeProps(c.intersection),
	rest: probeProps(c.rest),
};
