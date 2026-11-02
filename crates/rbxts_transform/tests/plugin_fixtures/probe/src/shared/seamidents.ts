// The names seam locals avoid are TypeScript 7.1's identifier set (`SourceFile::has_identifier`): JSDoc is not in it,
// and every string literal is.

/**
 * `impl` only in JSDoc: {@link impl}
 * @param impl a JSDoc parameter name
 */
export function documented(n: number) {
	return n;
}
// so the import local is `impl`
export const swapped = probeSwap(2, "b");
// a string literal anywhere takes its text: `constant_22_1`
export const text = "constant_22";
export const named = probeNamed([2]);
