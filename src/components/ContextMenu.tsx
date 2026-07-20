import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface MenuAction {
  id: string;
  label: string;
  icon?: React.ReactNode;
  danger?: boolean;
  run: () => void;
}

interface Props {
  x: number;
  y: number;
  actions: MenuAction[];
  onClose: () => void;
}

/**
 * A menu anchored at the pointer, used by both right-click and the ⋯ button.
 * Rendered fixed so it can escape card overflow, and flipped back on-screen
 * when it would spill past the window edge.
 */
export default function ContextMenu({ x, y, actions, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // Keep the menu fully on screen.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      x: Math.min(x, window.innerWidth - width - 8),
      y: Math.min(y, window.innerHeight - height - 8),
    });
  }, [x, y]);

  useEffect(() => {
    const close = () => onClose();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    // `capture` so the click that opens a different menu still closes this one.
    window.addEventListener("click", close, true);
    window.addEventListener("contextmenu", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", close, true);
      window.removeEventListener("contextmenu", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="context-menu"
      style={{ left: pos.x, top: pos.y }}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
    >
      {actions.map((a) => (
        <button
          key={a.id}
          className={`menu-item ${a.danger ? "menu-item-danger" : ""}`}
          onClick={() => {
            onClose();
            a.run();
          }}
        >
          {a.icon}
          {a.label}
        </button>
      ))}
    </div>
  );
}
