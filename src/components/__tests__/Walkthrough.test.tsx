import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { Walkthrough } from "../Walkthrough";

describe("Walkthrough component", () => {
  it("renders without crashing", () => {
    const html = renderToStaticMarkup(<Walkthrough onDismiss={() => {}} />);

    expect(html).toContain("Welcome to Right Now");
  });

  it("contains all key walkthrough points", () => {
    const html = renderToStaticMarkup(<Walkthrough onDismiss={() => {}} />);

    // Point 1: TODO.md is source of truth
    expect(html).toContain("TODO.md is your source of truth");
    expect(html).toContain("TODO.md");

    // Point 2: Edit in preferred editor
    expect(html).toContain("Edit in your preferred editor");
    expect(html).toContain("VS Code");
    expect(html).toContain("Vim");
    expect(html).toContain("Obsidian");

    // Point 3: Live file watching
    expect(html).toContain("Live file watching");
    expect(html).toContain("watches your file");
    expect(html).toContain("reloads automatically");

    // Point 4: Session badges and deep links
    expect(html).toContain("Session badges and deep links");
    expect(html).toContain("todos://session/");
  });

  it("has numbered steps 1-4", () => {
    const html = renderToStaticMarkup(<Walkthrough onDismiss={() => {}} />);

    expect(html).toContain(">1<");
    expect(html).toContain(">2<");
    expect(html).toContain(">3<");
    expect(html).toContain(">4<");
  });

  it("has a dismiss button with call to action", () => {
    const html = renderToStaticMarkup(<Walkthrough onDismiss={() => {}} />);

    // Note: apostrophe is HTML-encoded in SSR
    expect(html).toContain("Got it, let&#x27;s go!");
  });

  it("has a close X button", () => {
    const html = renderToStaticMarkup(<Walkthrough onDismiss={() => {}} />);

    expect(html).toContain("Close walkthrough");
  });
});
