import type { Tag } from "../../api/types";
import { tagColor } from "../tags";

interface TagChipProps {
  tag: Tag;
}

/** A tag: its word on its colour. The word is always there. */
export function TagChip({ tag }: TagChipProps) {
  const color = tagColor(tag.color);
  return (
    <span className="tag-chip" style={{ background: color.background, color: color.text }}>
      {tag.name}
    </span>
  );
}
