import type { MouseEvent, ReactNode } from "react";
import type { ExtraProps } from "react-markdown";
import { handleLink } from "../../lib/links";
import { useMarkdownContext } from "./MarkdownContext";

interface MarkdownLinkProps extends ExtraProps {
  href?: string;
  children?: ReactNode;
}

export function MarkdownLink({ href, children, node }: MarkdownLinkProps) {
  const { basePath } = useMarkdownContext();

  // Extract href from AST node if not passed directly
  const resolvedHref = href ?? (node?.properties?.href as string | undefined);

  const handleClick = async (e: MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault();
    if (resolvedHref) {
      const result = await handleLink(resolvedHref, basePath);
      if (!result.success) {
        console.error("Link handler error:", result.error);
      }
    }
  };

  return (
    <a
      href={resolvedHref}
      onClick={handleClick}
      className="text-blue-600 hover:text-blue-800 hover:underline cursor-pointer"
    >
      {children}
    </a>
  );
}
