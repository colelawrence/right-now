import { IconCheck, IconX } from "@tabler/icons-react";

interface WalkthroughProps {
  onDismiss: () => void;
}

export function Walkthrough({ onDismiss }: WalkthroughProps) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
        <div className="sticky top-0 bg-white border-b border-gray-200 px-6 py-4 flex items-center justify-between">
          <h2 className="text-xl font-semibold text-gray-900">Welcome to Right Now</h2>
          <button
            onClick={onDismiss}
            className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
            aria-label="Close walkthrough"
          >
            <IconX size={20} />
          </button>
        </div>

        <div className="px-6 py-6 space-y-6">
          <section className="space-y-3">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 bg-blue-100 text-blue-600 rounded-full flex items-center justify-center font-semibold text-sm">
                1
              </div>
              <div className="flex-1">
                <h3 className="font-semibold text-gray-900 mb-2">TODO.md is your source of truth</h3>
                <p className="text-sm text-gray-600 leading-relaxed">
                  Right Now works directly with your{" "}
                  <code className="px-1.5 py-0.5 bg-gray-100 rounded text-xs font-mono">TODO.md</code> file. No
                  databases, no exports—just a plain text file you can version control and read anywhere.
                </p>
              </div>
            </div>
          </section>

          <section className="space-y-3">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 bg-blue-100 text-blue-600 rounded-full flex items-center justify-center font-semibold text-sm">
                2
              </div>
              <div className="flex-1">
                <h3 className="font-semibold text-gray-900 mb-2">Edit in your preferred editor</h3>
                <p className="text-sm text-gray-600 leading-relaxed">
                  Use VS Code, Vim, Obsidian, or any text editor. Click the file path in the header to open your
                  TODO.md, make changes, save—and Right Now updates instantly.
                </p>
              </div>
            </div>
          </section>

          <section className="space-y-3">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 bg-blue-100 text-blue-600 rounded-full flex items-center justify-center font-semibold text-sm">
                3
              </div>
              <div className="flex-1">
                <h3 className="font-semibold text-gray-900 mb-2">Live file watching</h3>
                <p className="text-sm text-gray-600 leading-relaxed">
                  The app watches your file for changes and reloads automatically. Edit tasks, add notes, reorder items—
                  no manual refresh needed.
                </p>
              </div>
            </div>
          </section>

          <section className="space-y-3">
            <div className="flex items-start gap-3">
              <div className="flex-shrink-0 w-8 h-8 bg-blue-100 text-blue-600 rounded-full flex items-center justify-center font-semibold text-sm">
                4
              </div>
              <div className="flex-1">
                <h3 className="font-semibold text-gray-900 mb-2">Session badges and deep links</h3>
                <p className="text-sm text-gray-600 leading-relaxed">
                  When you complete a focus session, a badge appears next to your task with a clickable{" "}
                  <code className="px-1.5 py-0.5 bg-gray-100 rounded text-xs font-mono">todos://session/...</code> link.
                  Click it to see session details and notes.
                </p>
              </div>
            </div>
          </section>

          <div className="pt-4 border-t border-gray-200">
            <button
              onClick={onDismiss}
              className="w-full px-4 py-3 bg-blue-600 text-white font-medium rounded hover:bg-blue-700 transition-colors flex items-center justify-center gap-2"
            >
              <IconCheck size={20} />
              Got it, let's go!
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
