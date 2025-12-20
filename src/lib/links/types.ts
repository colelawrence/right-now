/**
 * Discriminated union for all supported link types
 */
export type LinkType =
  | { kind: "external"; url: string }
  | { kind: "todos-protocol"; action: string; params: Record<string, string> }
  | { kind: "file"; path: string }
  | { kind: "unknown"; raw: string };

/**
 * Result of handling a link
 */
export type LinkHandlerResult = { success: true } | { success: false; error: string };
