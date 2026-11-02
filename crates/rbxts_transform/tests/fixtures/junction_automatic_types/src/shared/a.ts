// A package imported by relative path, linked into the type root several times (tests/fixtures.rs,
// `junction_automatic_types`): `"types": ["*"]` makes every type root package an automatic type directive, and
// TypeScript meets their links in the type root's directory order: the import goes through the first, `alpha`.
import { greet } from "../../packages/greeter";

export const x = greet("t");
