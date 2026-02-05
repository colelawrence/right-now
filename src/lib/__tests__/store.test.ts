import { describe, expect, it } from "bun:test";
import { nextRecentProjects } from "../store";

describe("nextRecentProjects", () => {
  it("adds new project to empty list", () => {
    const result = nextRecentProjects([], "/path/to/project");
    expect(result).toEqual(["/path/to/project"]);
  });

  it("moves existing project to front", () => {
    const existing = ["/path/a", "/path/b", "/path/c"];
    const result = nextRecentProjects(existing, "/path/b");
    expect(result).toEqual(["/path/b", "/path/a", "/path/c"]);
  });

  it("adds new project to front of list", () => {
    const existing = ["/path/a", "/path/b"];
    const result = nextRecentProjects(existing, "/path/c");
    expect(result).toEqual(["/path/c", "/path/a", "/path/b"]);
  });

  it("limits list to default 10 items", () => {
    const existing = Array.from({ length: 10 }, (_, i) => `/path/${i}`);
    const result = nextRecentProjects(existing, "/path/new");
    expect(result.length).toBe(10);
    expect(result[0]).toBe("/path/new");
    expect(result[9]).toBe("/path/8"); // /path/9 dropped
  });

  it("respects custom limit", () => {
    const existing = ["/path/a", "/path/b", "/path/c"];
    const result = nextRecentProjects(existing, "/path/d", 2);
    expect(result).toEqual(["/path/d", "/path/a"]);
  });

  it("handles duplicate paths correctly", () => {
    const existing = ["/path/a", "/path/b", "/path/a", "/path/c"];
    const result = nextRecentProjects(existing, "/path/a");
    // Should deduplicate and move to front
    expect(result).toEqual(["/path/a", "/path/b", "/path/c"]);
  });

  it("preserves order of other projects when moving one to front", () => {
    const existing = ["/path/a", "/path/b", "/path/c", "/path/d"];
    const result = nextRecentProjects(existing, "/path/c");
    expect(result).toEqual(["/path/c", "/path/a", "/path/b", "/path/d"]);
  });
});
