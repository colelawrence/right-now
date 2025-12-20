import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import type { LinkHandlerResult, LinkType } from "./types";

/**
 * Detect and classify a link from its href string
 */
export function detectLinkType(href: string): LinkType {
  const trimmed = href.trim();

  // HTTP/HTTPS URLs → external
  if (/^https?:\/\//i.test(trimmed)) {
    return { kind: "external", url: trimmed };
  }

  // todos:// protocol
  if (trimmed.startsWith("todos://")) {
    const path = trimmed.slice("todos://".length);
    const sessionMatch = path.match(/^session\/(\d+)$/);
    if (sessionMatch) {
      return {
        kind: "todos-protocol",
        action: "session",
        params: { sessionId: sessionMatch[1] },
      };
    }
    // Unknown todos:// path
    return { kind: "unknown", raw: trimmed };
  }

  // file:// protocol
  if (trimmed.startsWith("file://")) {
    return { kind: "file", path: trimmed.slice("file://".length) };
  }

  // Absolute paths (Unix or Windows)
  if (/^\/[^/]/.test(trimmed) || /^[a-zA-Z]:\\/.test(trimmed)) {
    return { kind: "file", path: trimmed };
  }

  // Relative paths
  if (/^\.\.?\//.test(trimmed)) {
    return { kind: "file", path: trimmed };
  }

  return { kind: "unknown", raw: trimmed };
}

/**
 * Handle a link by detecting its type and routing to the appropriate handler
 * @param href - The link href to handle
 * @param basePath - Optional base directory for resolving relative paths
 */
export async function handleLink(href: string, basePath?: string): Promise<LinkHandlerResult> {
  const linkType = detectLinkType(href);

  switch (linkType.kind) {
    case "external":
      return handleExternalLink(linkType.url);

    case "todos-protocol":
      return handleTodosProtocol(linkType.action, linkType.params);

    case "file":
      return handleFilePath(linkType.path, basePath);

    case "unknown":
      return { success: false, error: `Unknown link type: ${linkType.raw}` };
  }
}

async function handleExternalLink(url: string): Promise<LinkHandlerResult> {
  try {
    await openUrl(url);
    return { success: true };
  } catch (error) {
    return { success: false, error: `Failed to open URL: ${error}` };
  }
}

function handleTodosProtocol(action: string, params: Record<string, string>): LinkHandlerResult {
  if (action === "session") {
    const sessionId = Number.parseInt(params.sessionId, 10);
    if (!Number.isNaN(sessionId)) {
      window.dispatchEvent(new CustomEvent("todos:session", { detail: { sessionId } }));
      return { success: true };
    }
  }
  return { success: false, error: `Unknown todos:// action: ${action}` };
}

/**
 * Resolve a potentially relative path against a base directory
 */
function resolvePath(path: string, basePath?: string): string {
  // If it's already absolute, return as-is
  if (/^\/[^/]/.test(path) || /^[a-zA-Z]:\\/.test(path)) {
    return path;
  }

  // If we have a base path and path is relative, join them
  if (basePath && /^\.\.?\//.test(path)) {
    // Remove trailing slash from basePath if present
    const base = basePath.endsWith("/") ? basePath.slice(0, -1) : basePath;
    // Handle ./ prefix
    if (path.startsWith("./")) {
      return `${base}/${path.slice(2)}`;
    }
    // Handle ../ prefix - simple implementation, go up one directory
    if (path.startsWith("../")) {
      const parentBase = base.split("/").slice(0, -1).join("/");
      return resolvePath(path.slice(3), parentBase);
    }
  }

  // If no base path but relative, we can't resolve it
  if (/^\.\.?\//.test(path) && !basePath) {
    throw new Error(`Cannot resolve relative path "${path}" without a base path`);
  }

  return path;
}

async function handleFilePath(path: string, basePath?: string): Promise<LinkHandlerResult> {
  try {
    const resolvedPath = resolvePath(path, basePath);
    await openPath(resolvedPath);
    return { success: true };
  } catch (error) {
    return { success: false, error: `Failed to open path: ${error}` };
  }
}
