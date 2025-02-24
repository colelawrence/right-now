import type { getDefaultStore } from "jotai";

export type JotaiStore = Pick<ReturnType<typeof getDefaultStore>, "get" | "set" | "sub">;
