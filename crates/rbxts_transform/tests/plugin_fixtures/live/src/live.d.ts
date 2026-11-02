/** A mark of `live_before_statement`'s answer (the `live` test plugin). */
declare function probeLive(): any;
/** `n` locals inserted before the statement. */
declare function probeInsert(n: number): void;
/** A local inserted before the root statement. */
declare function probeRoot(): void;
/** A hoist and a named spillable hoist. */
declare function probeHoist(): void;
/** A prerequisite local of the statement's own Luau; its value. */
declare function probePrereq(): number;
declare function use(...args: Array<unknown>): void;
declare const maybe: number | undefined;
declare const flag: boolean;
declare function probeOrder(): void;
