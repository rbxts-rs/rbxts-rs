// A `default` that is not the last clause. roblox-ts lowered it as the end of the switch: every case after it was
// dropped, so it ran the default where JavaScript runs a later case.

export function middle(x: number) {
	const log: string[] = [];
	switch (x) {
		case 1:
			log.push("one");
		default:
			log.push("default");
		case 2:
			log.push("two");
			break;
		case 3:
			log.push("three");
	}
	return log;
}

export function first(x: number) {
	const log: string[] = [];
	switch (x) {
		default:
			log.push("default");
			break;
		case 1:
		case 2:
			log.push("one or two");
	}
	return log;
}

// later cases are tested in order, once each, and only until one matches
export function testedOnce(x: number) {
	const log: string[] = [];
	const test = (value: number) => {
		log.push(`test ${value}`);
		return value;
	};
	switch (x) {
		case test(1):
			log.push("one");
			break;
		default:
			log.push("default");
			break;
		case test(2):
			log.push("two");
			break;
		case (log.push("prereq"), 3):
			log.push("three");
	}
	return log;
}

// A variable a `default` in the middle declares is one local of the case block, shared with the cases after it, as
// roblox-ts hoists a `case` clause's (`checkVariableHoist`); it was declared inside the default's `if`, so a later
// case wrote a global. (A later case reached without the default meets the variable in its TDZ in JavaScript;
// strict TypeScript lets it read the variable when its type includes `undefined`.)
export function sharedLet(x: number) {
	const log: string[] = [];
	switch (x) {
		case 1:
			log.push("one");
			break;
		default:
			let other: number | undefined = x * 10;
			log.push(`default ${other}`);
		// falls through
		case 3:
			other = (other ?? 0) + 1;
			log.push(`three ${other}`);
	}
	return log;
}

export function sharedConst(x: number) {
	const log: string[] = [];
	switch (x) {
		case 1:
			log.push("one");
		// falls through
		default:
			const doubled: number | undefined = x * 2;
			let later: number | undefined;
			log.push(`default ${doubled}`);
		// falls through
		case 3:
			later = (doubled ?? 0) + 1;
			log.push(`three ${doubled} ${later}`);
			break;
		case 4:
			log.push("four");
	}
	return log;
}

export function sharedDestructuring(x: number) {
	const log: string[] = [];
	switch (x) {
		case 1:
			log.push("one");
			break;
		default:
			const [first, second]: [number?, number?] = [x, x + 1];
			let { size, name }: { size?: number; name?: string } = { size: x * 3, name: `n${x}` };
		// falls through
		case 3:
			size = (size ?? 0) + 1;
			log.push(`${first} ${second} ${size} ${name}`);
	}
	return log;
}

// a variable the default alone uses stays inside its `if`
export function defaultOwn(x: number) {
	const log: string[] = [];
	switch (x) {
		default:
			const own = x + 1;
			log.push(`default ${own}`);
			break;
		case 3:
			const own3 = x + 3;
			log.push(`three ${own3}`);
	}
	return log;
}
