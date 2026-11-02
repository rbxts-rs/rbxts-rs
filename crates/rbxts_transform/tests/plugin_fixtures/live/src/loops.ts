// loops of both kinds: numeric (`$range`, optimized `for`), generic (arrays, sets, maps, strings, iterators), and
// the `while` lowerings
export function loops(list: Array<number>, set: Set<string>, map: Map<string, number>, text: string) {
	for (let i = 0; i < 10; i++) {
		probeLive();
		for (const j of $range(1, i)) {
			probeLive();
			use(j);
		}
	}
	for (let i = 0, n = list.size(); i < n; i += list[i]) {
		probeLive();
		use(i);
	}
	for (const item of list) {
		probeLive();
		use(item);
	}
	for (const key of set) {
		probeLive();
		use(key);
	}
	for (const [key, value] of map) {
		probeLive();
		use(key, value);
	}
	for (const pair of map) {
		probeLive();
		use(pair);
	}
	for (const character of text) {
		probeLive();
		use(character);
	}
	for (const [k, v] of pairs(list)) {
		probeLive();
		use(k, v);
	}
	for (const { a, b: [c, d = 2] } of [] as Array<{ a: number; b: [number, number?] }>) {
		probeLive();
		use(a, c, d);
	}
	for (const [x, , ...others] of [] as Array<Array<number>>) {
		probeLive();
		use(x, others);
	}
	for (const [s, t] of [] as Array<Set<number>>) {
		probeLive();
		use(s, t);
	}
	for (const value of list.size() > 0 ? list : [maybe ?? 0]) {
		probeLive();
		use(value);
	}
	while ((maybe ?? 0) > 1) {
		probeLive();
	}
	do {
		const inside = 1;
		probeLive();
		use(inside);
	} while (flag);
}
