// The declarations the probes use; `zz_late.ts` holds more, in a later file of the program.
export interface PA {
	x: number;
	y: number;
}
export interface PB extends PA {
	y: number;
	w: number;
}
export interface PC extends PB {
	c: number;
}
export interface First {
	f2: number;
	f1: number;
}
export enum Key {
	C = "c",
	A = "a",
	B = "b",
}
export interface Big {
	z: number;
	a: number;
	m: number;
	q?: string;
}
export class Base {
	b2 = 1;
	b1 = 2;
}
export interface Box<T> {
	v: T;
	k: string;
}
export interface UA {
	u2: number;
	u1: number;
	ua: number;
}
export declare const s1: unique symbol;
export declare const s0: unique symbol;
export function mkOne() {
	return { o2: 1, o1: 2 };
}
export function mkTwo() {
	return { o1: 1, o2: 2, o3: 3 };
}
