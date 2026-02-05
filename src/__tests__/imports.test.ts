/**
 * Import validation tests
 *
 * These tests verify that all module exports are properly defined.
 * This catches issues like:
 * - Missing files that are imported
 * - Undefined exports from barrel files
 * - Circular dependency issues causing undefined values
 *
 * Note: Modules that use Tauri APIs at load time cannot be tested here
 * since they require the browser/Tauri runtime environment.
 */
import { describe, expect, it } from "bun:test";

describe("Module exports are defined", () => {
  describe("components/markdown", () => {
    it("exports Markdown component", async () => {
      const { Markdown } = await import("../components/markdown");
      expect(Markdown).toBeDefined();
      expect(typeof Markdown).toBe("function");
    });

    it("exports MarkdownLink component", async () => {
      const { MarkdownLink } = await import("../components/markdown");
      expect(MarkdownLink).toBeDefined();
      expect(typeof MarkdownLink).toBe("function");
    });
  });

  describe("lib/ProjectStateEditor", () => {
    it("exports ProjectStateEditor namespace with parse function", async () => {
      const { ProjectStateEditor } = await import("../lib/ProjectStateEditor");
      expect(ProjectStateEditor).toBeDefined();
      expect(typeof ProjectStateEditor.parse).toBe("function");
    });

    it("exports formatSessionBadge", async () => {
      const { formatSessionBadge } = await import("../lib/ProjectStateEditor");
      expect(formatSessionBadge).toBeDefined();
      expect(typeof formatSessionBadge).toBe("function");
    });
  });

  describe("components/utils", () => {
    it("exports cn utility", async () => {
      const { cn } = await import("../components/utils/cn");
      expect(cn).toBeDefined();
      expect(typeof cn).toBe("function");
    });
  });

  /**
   * This test would have caught the blank screen bug.
   * It verifies that React components are actually functions that can be called,
   * not undefined due to missing files or broken exports.
   */
  describe("React components are callable", () => {
    it("Markdown can be called as a function", async () => {
      const { Markdown } = await import("../components/markdown");
      // React components must be functions (or classes)
      // If the export is undefined, this test fails
      expect(() => {
        // Just verify it's callable - don't actually render
        const result = Markdown({ children: "test" });
        expect(result).toBeDefined();
      }).not.toThrow();
    });

    it("Walkthrough can be called as a function", async () => {
      const { Walkthrough } = await import("../components/Walkthrough");
      expect(() => {
        const result = Walkthrough({ onDismiss: () => {} });
        expect(result).toBeDefined();
      }).not.toThrow();
    });
  });
});
