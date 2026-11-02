/** @probe props */
declare function probeProps(a: unknown): string;

// one global interface declared in two files (`merged2.d.ts`): its members in the order of its declarations' files
interface Merged {
	m2: number;
	m1: number;
}
