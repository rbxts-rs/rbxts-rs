// switch (fallthrough flag, a default in the middle, shared case variables, `continue` through it) and try
export function control(n: number, list: Array<number>) {
	switch (n) {
		case 1:
		case 2: {
			probeLive();
			break;
		}
		case 3:
			probeLive();
		// falls through
		case 4:
			probeLive();
			break;
	}
	switch (n + 1) {
		case 1:
			probeLive();
			break;
		default:
			probeLive();
			break;
		case 2:
			probeLive();
	}
	switch (n) {
		case 1:
			let shared = 1;
			probeLive();
			break;
		case 2:
			shared = 2;
			probeLive();
			use(shared);
			break;
	}
	// a default in the middle shares its variables with the cases after it (`local other` before its `if`)
	switch (n) {
		case 1:
			probeLive();
			break;
		default:
			let other = n;
			const own = 2;
			probeLive();
			use(other, own);
		// falls through
		case 3:
			other = 3;
			probeLive();
			use(other);
	}
	for (const value of list) {
		switch (value) {
			case 0:
				probeLive();
				continue;
		}
		probeLive();
	}
	try {
		const attempt = n;
		probeLive();
		use(attempt);
	} catch (error) {
		probeLive();
		use(error);
	} finally {
		probeLive();
	}
}

// plugin insertions and prerequisites before a mark
export function insertions() {
	use(probeInsert(2), probeLive());
	const value = probePrereq();
	probeLive();
	use(value, probeHoist(), probeLive());
	return value;
}

export namespace Space {
	export const member = 1;
	probeLive();
	export namespace Inner {
		probeLive();
	}
}
