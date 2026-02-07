import { IconChevronDown } from "@tabler/icons-react";
import { useEffect, useId, useRef, useState } from "react";
import { cn } from "./utils/cn";

type Variant = "header" | "welcome";

export type OpenProjectSplitButtonProps = {
  onOpenFile: () => void;
  onOpenFolder: () => void;
  onShowWalkthrough?: () => void;
  variant?: Variant;
};

const styles: Record<Variant, { group: string; main: string; chevron: string; menu: string; item: string }> = {
  header: {
    group: "inline-flex",
    main: "text-xs px-2.5 py-1 text-gray-600 hover:bg-gray-100 transition-colors rounded-l-md",
    chevron: "p-1 text-gray-600 hover:bg-gray-100 transition-colors rounded-r-md",
    menu: "absolute right-0 mt-1 min-w-[12rem] rounded-md border border-gray-200 bg-white shadow-lg z-50 py-1",
    item: "w-full px-3 py-2 text-left text-xs text-gray-700 hover:bg-gray-50",
  },
  welcome: {
    group: "inline-flex shadow-sm",
    main: "px-5 py-2.5 bg-blue-600 text-white text-sm hover:bg-blue-700 transition-all hover:shadow-md active:scale-95 rounded-l-md",
    chevron:
      "px-2.5 py-2.5 bg-blue-600 text-white text-sm hover:bg-blue-700 transition-all hover:shadow-md active:scale-95 rounded-r-md border-l border-blue-500/50",
    menu: "absolute right-0 mt-2 min-w-[12rem] rounded-md border border-gray-200 bg-white shadow-lg z-50 py-1",
    item: "w-full px-3 py-2 text-left text-sm text-gray-700 hover:bg-gray-50",
  },
};

export function OpenProjectSplitButton({
  onOpenFile,
  onOpenFolder,
  onShowWalkthrough,
  variant = "header",
}: OpenProjectSplitButtonProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;

    const onDocMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (rootRef.current?.contains(target)) return;
      setOpen(false);
    };

    const onDocKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };

    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onDocKeyDown);

    return () => {
      document.removeEventListener("mousedown", onDocMouseDown);
      document.removeEventListener("keydown", onDocKeyDown);
    };
  }, [open]);

  const s = styles[variant];

  const closeThen = (fn: () => void) => {
    setOpen(false);
    fn();
  };

  return (
    <div
      ref={rootRef}
      className="relative"
      // Prevent titlebar dragging from swallowing interactions.
      onMouseDown={(event) => event.stopPropagation()}
    >
      <div className={s.group}>
        <button type="button" onClick={onOpenFile} className={s.main} title="Open TODO file">
          Open File...
        </button>
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className={s.chevron}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-controls={menuId}
          title="Open options"
        >
          <IconChevronDown size={16} />
        </button>
      </div>

      {open && (
        <div id={menuId} role="menu" className={s.menu}>
          <button type="button" role="menuitem" className={s.item} onClick={() => closeThen(onOpenFile)}>
            Open File...
          </button>
          <button type="button" role="menuitem" className={s.item} onClick={() => closeThen(onOpenFolder)}>
            Open Folder...
          </button>

          {onShowWalkthrough && (
            <>
              <div className={cn("my-1 h-px bg-gray-200", variant === "welcome" && "my-2")} role="separator" />
              <button type="button" role="menuitem" className={s.item} onClick={() => closeThen(onShowWalkthrough)}>
                Show walkthrough
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
