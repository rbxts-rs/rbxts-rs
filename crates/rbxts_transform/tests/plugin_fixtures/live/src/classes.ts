// classes: field initializers (moved into the constructor), statements after `super`, methods, static blocks
class Base {
	constructor(public readonly base: number) {}
}

export class Derived extends Base {
	field = maybe ?? 1;
	marked = probeLive();
	other = [this.field, probePrereq()];

	constructor(value: number, { option }: { option: number }) {
		const before = value + option;
		probeLive();
		super(before);
		probeLive();
		const after = this.field;
		probeLive();
		use(after);
	}

	method(argument: number) {
		probeLive();
		return argument;
	}

	static {
		const shared = 1;
		probeLive();
		use(shared);
	}
}

export const anonymous = class {
	value = probeLive();
};
