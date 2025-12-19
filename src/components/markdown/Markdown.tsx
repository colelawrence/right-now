import type React from "react";
import type { ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "../utils/cn";
import { MarkdownLink } from "./MarkdownLink";

interface MarkdownProps {
  children: string;
  /** Render inline only (unwraps block elements like <p>) */
  inline?: boolean;
  className?: string;
}

// Custom URL transform that allows todos:// and file:// protocols
// in addition to the defaults (http, https, mailto, etc.)
const allowedProtocols = /^(https?|ircs?|mailto|xmpp|todos|file)$/i;

export function urlTransform(url: string): string {
  const colon = url.indexOf(":");
  if (colon !== -1) {
    const protocol = url.slice(0, colon);
    if (allowedProtocols.test(protocol)) {
      return url;
    }
  }
  // Fall back to default transform for other URLs
  return defaultUrlTransform(url);
}

export function Markdown({ children, inline, className }: MarkdownProps) {
  // Build components object, only including overrides when needed
  const components: Record<string, React.ComponentType<{ children?: ReactNode }>> = {
    a: MarkdownLink,
  };

  // For inline mode, unwrap paragraphs by rendering just the children
  if (inline) {
    components.p = ({ children }: { children?: ReactNode }) => <>{children}</>;
  }

  return (
    <span className={cn("markdown-content", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components} urlTransform={urlTransform}>
        {children}
      </ReactMarkdown>
    </span>
  );
}
