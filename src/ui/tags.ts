// Fixed tag colours. Each pairs a dark background with white text, at least 7:1 contrast
// (checked in tags.test.ts), and the tag's word is always shown so colour never carries the meaning.

import type { MessageKey } from "../i18n";

export interface TagColor {
  key: string;
  background: string;
  text: string;
  label: MessageKey;
}

export const TAG_PALETTE: readonly TagColor[] = [
  { key: "red", background: "#8f1d1d", text: "#ffffff", label: "tag.color.red" },
  { key: "orange", background: "#8a3b00", text: "#ffffff", label: "tag.color.orange" },
  { key: "green", background: "#1e5626", text: "#ffffff", label: "tag.color.green" },
  { key: "blue", background: "#123f84", text: "#ffffff", label: "tag.color.blue" },
  { key: "purple", background: "#55267a", text: "#ffffff", label: "tag.color.purple" },
  { key: "brown", background: "#5b3a1e", text: "#ffffff", label: "tag.color.brown" },
  { key: "gray", background: "#3d3d3d", text: "#ffffff", label: "tag.color.gray" },
];

export function tagColor(key: string): TagColor {
  return TAG_PALETTE.find((c) => c.key === key) ?? TAG_PALETTE[TAG_PALETTE.length - 1];
}

function channel(hex: string, start: number): number {
  const v = parseInt(hex.slice(start, start + 2), 16) / 255;
  return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  return 0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5);
}

/** WCAG contrast ratio of two #rrggbb colours. */
export function contrastRatio(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
