// A small set of line icons (drawn for Keylume, 24×24, stroke = currentColor).

const PATHS = {
  library: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  create: "M12 5v14M5 12h14",
  wand: "M4 20L15 9M14 4v3M18.5 5.5l-2 2M20 10h-3M9 4l.5 1.5L11 6l-1.5.5L9 8l-.5-1.5L7 6l1.5-.5z",
  strip: "M5 4h3v16H5zM10.5 7h3v10h-3zM16 10h3v4h-3z",
  keyboard: "M3 7h18v11H3zM6 10h1M9 10h1M12 10h1M15 10h1M18 10h1M6 13h1M18 13h1M9 13h6M7 15.5h10",
  macro: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18zM12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z",
  chip: "M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4",
  settings: "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6zM12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1",
  star: "M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.8l-5.2 2.8 1-5.8-4.3-4.1 5.9-.9z",
  search: "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14zM16 16l4.5 4.5",
  bulb: "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2V16h5v-.1c0-.8.4-1.5 1-2A6 6 0 0 0 12 3z",
  gamepad: "M6 8h12a4 4 0 0 1 4 4v1a4 4 0 0 1-7 2.6l-.6-.6H9.6l-.6.6A4 4 0 0 1 2 13v-1a4 4 0 0 1 4-4zM7 11v3M5.5 12.5h3M16 11.5h.01M18 13.5h.01",
  play: "M8 5l11 7-11 7z",
  stop: "M7 7h10v10H7z",
  edit: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
  check: "M5 12.5l4.5 4.5L19 7",
  paint: "M4 20c3 0 4-2 4-4a2 2 0 0 0-4 0M9 15L20 4M13 11l2 2",
  bolt: "M13 2L4 14h7l-1 8 9-12h-7z",
  text: "M5 6h14M12 6v13M9 19h6",
  upload: "M12 16V4M7 9l5-5 5 5M4 20h16",
  home: "M4 11l8-7 8 7M6 9.5V20h4.5v-5.5h3V20H18V9.5",
  power: "M12 3v8M7.1 6.3a7 7 0 1 0 9.8 0",
  sliders: "M5 4v16M12 4v16M19 4v16M3 8h4M10 15h4M17 10h4",
  close: "M6 6l12 12M18 6L6 18",
  left: "M15 5l-7 7 7 7",
  right: "M9 5l7 7-7 7",
  user: "M12 4a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM4.5 20a7.5 7.5 0 0 1 15 0",
  undo: "M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11",
  pack: "M4 7.5l8-4 8 4v9l-8 4-8-4zM4 7.5l8 4 8-4M12 11.5v9",
  redo: "M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 18, className }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg className={`icon-svg ${className ?? ""}`} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d={PATHS[name]} />
    </svg>
  );
}
