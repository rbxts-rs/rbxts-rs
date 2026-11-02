// the main chunk: an import, hoists, root insertions and top-level blocks
import { helper } from "./functions";

probeLive();
const top = helper(1);
probeHoist();
probeLive();
probeRoot();
const other = maybe ?? top;
probeLive();
if (flag) {
	const inner = top + other;
	probeLive();
	use(inner);
} else if (probeLive()) {
	probeLive();
}
for (const value of [1, 2, 3]) {
	probeLive();
	use(value);
}
export const exported = probeLive();
