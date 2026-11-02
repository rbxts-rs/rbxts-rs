// A TypeScript 5 `baseUrl` project moved to TypeScript 7 (tests/watch.rs, `baseurl`): a name only the `"*"` pattern
// resolves, a `paths` pattern, and a declaration that names a type of a module it does not import.
import { m } from "shared/mod";
import { t, getThing } from "@lib/thing";

export const a = m + t;
export const thing = getThing();
