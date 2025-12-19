import { describe, expect, it } from "bun:test";
import { urlTransform } from "../Markdown";

describe("urlTransform", () => {
  describe("todos:// protocol", () => {
    it("allows todos:// links without trailing content", () => {
      const url = "todos://session/0";
      expect(urlTransform(url)).toBe(url);
    });

    it("allows todos:// links with path segments", () => {
      const url = "todos://session/123";
      expect(urlTransform(url)).toBe(url);
    });

    it("preserves the full todos:// URL", () => {
      const url = "todos://some/deep/path";
      expect(urlTransform(url)).toBe(url);
    });
  });

  describe("file:// protocol", () => {
    it("allows file:// links", () => {
      const url = "file:///path/to/file.txt";
      expect(urlTransform(url)).toBe(url);
    });
  });

  describe("standard protocols", () => {
    it("allows https:// links", () => {
      const url = "https://example.com";
      expect(urlTransform(url)).toBe(url);
    });

    it("allows http:// links", () => {
      const url = "http://example.com";
      expect(urlTransform(url)).toBe(url);
    });

    it("allows mailto: links", () => {
      const url = "mailto:test@example.com";
      expect(urlTransform(url)).toBe(url);
    });
  });

  describe("relative URLs", () => {
    it("allows relative paths", () => {
      const url = "./relative/path";
      expect(urlTransform(url)).toBe(url);
    });

    it("allows parent directory paths", () => {
      const url = "../parent/path";
      expect(urlTransform(url)).toBe(url);
    });

    it("allows root-relative paths", () => {
      const url = "/absolute/path";
      expect(urlTransform(url)).toBe(url);
    });
  });

  describe("unsafe protocols", () => {
    it("blocks javascript: URLs", () => {
      const url = "javascript:alert(1)";
      expect(urlTransform(url)).toBe("");
    });

    it("blocks data: URLs", () => {
      const url = "data:text/html,<script>alert(1)</script>";
      expect(urlTransform(url)).toBe("");
    });

    it("blocks vbscript: URLs", () => {
      const url = "vbscript:msgbox(1)";
      expect(urlTransform(url)).toBe("");
    });
  });
});
