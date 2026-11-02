// functions: parameters, defaults, destructuring, rest, methods, nested functions, generators
export function helper(a: number, b = 2, ...rest: Array<number>) {
	probeLive();
	const c = a + b;
	const inner = (x: number, { y, z }: { y: number; z: number }) => {
		probeLive();
		return x + y + z + c;
	};
	probeLive();
	return inner(c, { y: rest.size(), z: 1 });
}

export const object = {
	method(this: object, p: number) {
		probeLive();
		return p;
	},
	arrow: ([first, second]: [number, number]) => {
		probeLive();
		return first + second;
	},
};

export function* generator(n: number) {
	probeLive();
	yield n;
}

export async function asynchronous(n: number) {
	const doubled = n * 2;
	probeLive();
	return doubled;
}

export function hoisting() {
	if (flag) {
		probeLive();
		later();
	}
	const value = 1;
	while (value > 0) {
		probeLive();
		laterToo(value);
	}
	function later() {}
	function laterToo(v: number) {
		probeLive();
		return v;
	}
}

export function arrowBody() {
	const f = () => probeLive();
	return f;
}
