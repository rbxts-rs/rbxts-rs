import type { Cases } from "./cases";

declare const c: Cases;
// file 3 of 8: the same probes in another order (from `classes`), so each checker
// meets the types in another order
export const lists = {
	classes: probeProps(c.classes),
	merged: probeProps(c.merged),
	indexFirst: probeProps(c.indexFirst),
	symbols: probeProps(c.symbols),
	returned: probeProps(c.returned),
	chain: probeProps(c.chain),
	both: probeProps(c.both),
	literalUnion: probeProps(c.literalUnion),
	interfaceUnion: probeProps(c.interfaceUnion),
	interfaceUnionReversed: probeProps(c.interfaceUnionReversed),
	recordOfKeys: probeProps(c.recordOfKeys),
	recordOfEnum: probeProps(c.recordOfEnum),
	recordOfNumbers: probeProps(c.recordOfNumbers),
	recordOfTemplate: probeProps(c.recordOfTemplate),
	pick: probeProps(c.pick),
	mappedKeys: probeProps(c.mappedKeys),
	mappedRenamed: probeProps(c.mappedRenamed),
	partial: probeProps(c.partial),
	omit: probeProps(c.omit),
	readonly: probeProps(c.readonly),
	spread: probeProps(c.spread),
	rest: probeProps(c.rest),
	intersection: probeProps(c.intersection),
	intersectionReversed: probeProps(c.intersectionReversed),
	box: probeProps(c.box),
	boxUnion: probeProps(c.boxUnion),
};
