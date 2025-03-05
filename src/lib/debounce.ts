export function debounce<T extends (...args: any[]) => void>(spaceMs: number, fn: T) {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  return Object.assign(
    (...args: Parameters<T>) => {
      clearTimeout(timeout);
      timeout = setTimeout(fn, spaceMs, ...args);
    },
    { cancel: () => clearTimeout(timeout) },
  );
}
