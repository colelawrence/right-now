import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useEffect } from "react";
import { handleLink } from "./links";

/**
 * Hook to handle incoming deep link URLs (todos:// protocol).
 *
 * On macOS, URLs are received via events.
 * On Windows/Linux, URLs are passed as CLI arguments on startup.
 *
 * @param basePath - Optional base path for resolving relative paths in links
 */
export function useDeepLink(basePath?: string) {
  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      try {
        // Listen for incoming deep link URLs (macOS sends events)
        unlistenFn = await onOpenUrl((urls) => {
          for (const url of urls) {
            console.log("[deep-link] Received URL:", url);
            handleLink(url, basePath).then((result) => {
              if (!result.success) {
                console.error("[deep-link] Failed to handle URL:", result.error);
              }
            });
          }
        });
      } catch (error) {
        // Deep link plugin may not be available in dev mode or certain environments
        console.warn("[deep-link] Failed to setup listener:", error);
      }
    };

    setupListener();

    return () => {
      unlistenFn?.();
    };
  }, [basePath]);
}
