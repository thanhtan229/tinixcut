import { useLayoutEffect, useRef } from "react";

/** Invalidates a pending review even if its context changes and later changes back. */
export function useReviewContext(key: string, isOpen: boolean, onChange: () => void) {
  const context = useRef({ key, version: 0, isOpen });
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  useLayoutEffect(() => {
    if (context.current.key === key) return;
    context.current = { key, version: context.current.version + 1, isOpen };
    onChangeRef.current();
  }, [key, isOpen]);
  return context;
}
