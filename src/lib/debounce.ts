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

export function throttle<T extends (...args: any[]) => void>(spaceMs: number, fn: T) {
  let last = 0;
  return Object.assign(
    (...args: Parameters<T>) => {
      const now = Date.now();
      if (now - last > spaceMs) {
        last = now;
        fn(...args);
      }
    },
    {
      clear: () => {
        last = 0;
      },
    },
  );
}

export function debounceAndThrottle<T extends (...args: any[]) => void>(debounceMs: number, throttleMs: number, fn: T) {
  const throttled = throttle(throttleMs, fn);
  const debounced = debounce(debounceMs, throttled);
  return Object.assign(debounced, { clearThrottle: throttled.clear });
}
