import { describe, expect, it } from "bun:test";

describe("Startup auto-load behavior", () => {
  it("should extract filename from full path for error message", () => {
    // Simulates what happens in main.tsx when auto-load fails
    const lastProject = "/Users/test/Documents/projects/my-project/TODO.md";
    const errorMessage = `Could not load previous project: ${lastProject.split("/").pop()}`;

    expect(errorMessage).toBe("Could not load previous project: TODO.md");
  });

  it("should handle paths without directory separators", () => {
    const lastProject = "TODO.md";
    const errorMessage = `Could not load previous project: ${lastProject.split("/").pop()}`;

    expect(errorMessage).toBe("Could not load previous project: TODO.md");
  });

  it("should handle empty path edge case", () => {
    const lastProject = "";
    const errorMessage = `Could not load previous project: ${lastProject.split("/").pop()}`;

    // Empty string split by "/" returns [""], so pop() returns ""
    expect(errorMessage).toBe("Could not load previous project: ");
  });
});
