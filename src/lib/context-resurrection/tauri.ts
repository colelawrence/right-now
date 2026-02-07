import { invoke } from "@tauri-apps/api/core";
import type { CrTransport } from "./client";
import type { CrDaemonRequest, CrDaemonResponse } from "./types";

/**
 * Production transport that proxies CR daemon protocol requests through Tauri.
 *
 * Requires the Rust command `cr_request` to be registered.
 */
export function createTauriCrTransport(): CrTransport {
  return async (request: CrDaemonRequest) => {
    return await invoke<CrDaemonResponse>("cr_request", { request });
  };
}
