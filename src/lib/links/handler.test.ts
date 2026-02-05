import { describe, expect, test } from "bun:test";
import { detectLinkType } from "./handler";

describe("detectLinkType", () => {
  test("recognizes todos://session/<id> links", () => {
    const result = detectLinkType("todos://session/42");
    expect(result).toEqual({
      kind: "todos-protocol",
      action: "session",
      params: { sessionId: "42" },
    });
  });

  test("recognizes todos://session/<id> with larger IDs", () => {
    const result = detectLinkType("todos://session/123456");
    expect(result).toEqual({
      kind: "todos-protocol",
      action: "session",
      params: { sessionId: "123456" },
    });
  });

  test("rejects invalid todos://session paths", () => {
    expect(detectLinkType("todos://session/")).toEqual({
      kind: "unknown",
      raw: "todos://session/",
    });

    expect(detectLinkType("todos://session/abc")).toEqual({
      kind: "unknown",
      raw: "todos://session/abc",
    });

    expect(detectLinkType("todos://session/42/extra")).toEqual({
      kind: "unknown",
      raw: "todos://session/42/extra",
    });
  });

  test("recognizes HTTP/HTTPS URLs", () => {
    const result = detectLinkType("https://example.com");
    expect(result).toEqual({
      kind: "external",
      url: "https://example.com",
    });
  });

  test("recognizes file paths", () => {
    expect(detectLinkType("/absolute/path")).toEqual({
      kind: "file",
      path: "/absolute/path",
    });

    expect(detectLinkType("./relative/path")).toEqual({
      kind: "file",
      path: "./relative/path",
    });

    expect(detectLinkType("../parent/path")).toEqual({
      kind: "file",
      path: "../parent/path",
    });
  });

  test("recognizes file:// protocol", () => {
    const result = detectLinkType("file:///absolute/path");
    expect(result).toEqual({
      kind: "file",
      path: "/absolute/path",
    });
  });

  test("handles unknown link types", () => {
    const result = detectLinkType("unknown-protocol://something");
    expect(result).toEqual({
      kind: "unknown",
      raw: "unknown-protocol://something",
    });
  });

  test("trims whitespace from hrefs", () => {
    const result = detectLinkType("  todos://session/42  ");
    expect(result).toEqual({
      kind: "todos-protocol",
      action: "session",
      params: { sessionId: "42" },
    });
  });
});
