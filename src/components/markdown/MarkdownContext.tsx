import { createContext, useContext } from "react";

interface MarkdownContextValue {
  /** Base directory for resolving relative paths (directory containing the markdown file) */
  basePath?: string;
}

const MarkdownContext = createContext<MarkdownContextValue>({});

export function MarkdownProvider({
  basePath,
  children,
}: {
  basePath?: string;
  children: React.ReactNode;
}) {
  return <MarkdownContext.Provider value={{ basePath }}>{children}</MarkdownContext.Provider>;
}

export function useMarkdownContext() {
  return useContext(MarkdownContext);
}
