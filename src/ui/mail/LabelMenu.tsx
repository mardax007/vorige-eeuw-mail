import { useState } from "react";
import { api } from "../../api/client";
import type { Tag } from "../../api/types";
import { t } from "../../i18n";
import { TagChip } from "../common/TagChip";
import { TAG_PALETTE } from "../tags";

interface LabelMenuProps {
  messageId: string;
  tags: readonly Tag[];
  activeTagIds: readonly string[];
  onChanged: () => void;
  onClose: () => void;
  onError: (e: unknown) => void;
}

/** A simple list: press a label to put it on or take it off. Plus "Nieuw label". */
export function LabelMenu({ messageId, tags, activeTagIds, onChanged, onClose, onError }: LabelMenuProps) {
  const [name, setName] = useState("");
  const [color, setColor] = useState(TAG_PALETTE[0].key);

  const toggle = async (tag: Tag) => {
    try {
      await api.setMessageTags([messageId], tag.id, !activeTagIds.includes(tag.id));
      onChanged();
    } catch (e: unknown) {
      onError(e);
    }
  };

  const create = async () => {
    const trimmed = name.trim();
    if (trimmed === "") return;
    try {
      const tag = await api.createTag(trimmed, color);
      await api.setMessageTags([messageId], tag.id, true);
      setName("");
      onChanged();
    } catch (e: unknown) {
      onError(e);
    }
  };

  return (
    <div className="popup-panel" role="group" aria-label={t("tag.title")}>
      <h3>{t("tag.title")}</h3>
      <ul className="choice-list">
        {tags.map((tag) => {
          const isOn = activeTagIds.includes(tag.id);
          return (
            <li key={tag.id}>
              <label className="check-row">
                <input type="checkbox" checked={isOn} onChange={() => toggle(tag)} />
                <TagChip tag={tag} />
              </label>
            </li>
          );
        })}
      </ul>
      <h4>{t("tag.new")}</h4>
      <div className="form-row">
        <label htmlFor="new-tag-name">{t("tag.name")}</label>
        <input id="new-tag-name" value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div className="form-row">
        <label htmlFor="new-tag-color">{t("tag.color")}</label>
        <select id="new-tag-color" value={color} onChange={(e) => setColor(e.target.value)}>
          {TAG_PALETTE.map((c) => (
            <option key={c.key} value={c.key}>
              {t(c.label)}
            </option>
          ))}
        </select>
      </div>
      <div className="button-row">
        <button type="button" onClick={create} disabled={name.trim() === ""}>
          {t("tag.create")}
        </button>
        <button type="button" className="primary" onClick={onClose}>
          {t("tag.close")}
        </button>
      </div>
    </div>
  );
}
