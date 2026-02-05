import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { ProjectManager, ProjectStore } from "../../lib";
import { Welcome } from "../Welcome";

describe("Welcome component", () => {
  const createMockProjectManager = (): ProjectManager => {
    return {
      openProject: () => {},
      openProjectFolder: () => {},
      loadProject: () => Promise.resolve(),
    } as unknown as ProjectManager;
  };

  const createMockProjectStore = (recentProjects: string[] = []): ProjectStore => {
    return {
      getRecentProjects: () => Promise.resolve(recentProjects),
    } as unknown as ProjectStore;
  };

  it("renders without crashing", () => {
    const projectManager = createMockProjectManager();
    const projectStore = createMockProjectStore();

    const html = renderToStaticMarkup(
      <Welcome projectManager={projectManager} projectStore={projectStore} onShowWalkthrough={() => {}} />,
    );

    expect(html).toContain("Welcome to Right Now");
  });

  it("renders Open File and Open Folder buttons", () => {
    const projectManager = createMockProjectManager();
    const projectStore = createMockProjectStore();

    const html = renderToStaticMarkup(
      <Welcome projectManager={projectManager} projectStore={projectStore} onShowWalkthrough={() => {}} />,
    );

    expect(html).toContain("Open File...");
    expect(html).toContain("Open Folder...");
  });

  it("renders startup warning when provided", () => {
    const projectManager = createMockProjectManager();
    const projectStore = createMockProjectStore();
    const startupWarning = {
      message: "Test warning",
      details: "Test details",
    };

    const html = renderToStaticMarkup(
      <Welcome
        projectManager={projectManager}
        projectStore={projectStore}
        startupWarning={startupWarning}
        onShowWalkthrough={() => {}}
      />,
    );

    expect(html).toContain("Test warning");
    expect(html).toContain("Test details");
  });

  // Note: The following tests can't verify async-loaded recent projects with renderToStaticMarkup
  // since useEffect doesn't run in SSR. These would require a proper React testing library setup
  // with DOM and async utilities. For now, we keep the tests minimal and focused on basic rendering.

  it("does not crash when store returns recent projects", () => {
    const projectManager = createMockProjectManager();
    const projectStore = createMockProjectStore(["/Users/test/project1/TODO.md", "/Users/test/project2/TODO.md"]);

    // Should render without throwing, even though recent projects won't appear in SSR
    const html = renderToStaticMarkup(
      <Welcome projectManager={projectManager} projectStore={projectStore} onShowWalkthrough={() => {}} />,
    );

    expect(html).toContain("Welcome to Right Now");
  });

  it("renders show walkthrough button", () => {
    const projectManager = createMockProjectManager();
    const projectStore = createMockProjectStore();

    const html = renderToStaticMarkup(
      <Welcome projectManager={projectManager} projectStore={projectStore} onShowWalkthrough={() => {}} />,
    );

    expect(html).toContain("Show walkthrough");
  });
});
