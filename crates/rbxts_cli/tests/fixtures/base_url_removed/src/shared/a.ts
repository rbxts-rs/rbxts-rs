// `baseUrl` (tests/cli.rs, `base_url_removed`): TypeScript 7 removed it (TS5102), so the names only it resolved
// fail too (TS2307).
import { m } from "shared/mod";
import { t } from "@lib/thing";
import { f } from "@lib/fallback";

export const a = m + t + f;
