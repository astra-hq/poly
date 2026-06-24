'use client';

import { useCallback, useEffect, useRef } from 'react';

interface UseLongPressOptions {
  onLongPress: () => void;
  onClick?: () => void;
  delay?: number;
  moveThreshold?: number;
}

interface UseLongPressReturn {
  onPointerDown: (e: React.PointerEvent) => void;
  onPointerUp: () => void;
  onPointerLeave: () => void;
  onPointerCancel: () => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onClick: (e: React.MouseEvent) => void;
  onContextMenu: (e: React.MouseEvent) => void;
}

export function useLongPress({
  onLongPress,
  onClick,
  delay = 500,
  moveThreshold = 10,
}: UseLongPressOptions): UseLongPressReturn {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const triggeredRef = useRef(false);
  const startPosRef = useRef({ x: 0, y: 0 });

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      clearTimer();
      triggeredRef.current = false;
      startPosRef.current = { x: e.clientX, y: e.clientY };
      timerRef.current = setTimeout(() => {
        triggeredRef.current = true;
        onLongPress();
      }, delay);
    },
    [clearTimer, onLongPress, delay],
  );

  const onPointerUp = useCallback(() => {
    clearTimer();
  }, [clearTimer]);

  const onPointerLeave = useCallback(() => {
    clearTimer();
  }, [clearTimer]);

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const dx = Math.abs(e.clientX - startPosRef.current.x);
      const dy = Math.abs(e.clientY - startPosRef.current.y);
      if (dx > moveThreshold || dy > moveThreshold) {
        clearTimer();
      }
    },
    [clearTimer, moveThreshold],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (triggeredRef.current) {
        triggeredRef.current = false;
        e.preventDefault();
        e.stopPropagation();
        return;
      }
      onClick?.();
    },
    [onClick],
  );

  const onContextMenu = useCallback((e: React.MouseEvent) => {
    if (triggeredRef.current) {
      e.preventDefault();
    }
  }, []);

  useEffect(() => clearTimer, [clearTimer]);

  return {
    onPointerDown,
    onPointerUp,
    onPointerLeave,
    onPointerCancel: onPointerLeave,
    onPointerMove,
    onClick: handleClick,
    onContextMenu,
  };
}
