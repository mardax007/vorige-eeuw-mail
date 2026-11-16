import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { ALL_INBOXES_ID } from "../../api/commands";
import type { Folder, SearchHit, SearchScope } from "../../api/types";
import { t, type MessageKey } from "../../i18n";
import type { MailLocation } from "../state/AppContext";

export const SEARCH_DEBOUNCE_MS = 150;
export const SEARCH_LIMIT = 200;

export type ScopeChoice = "all" | "account" | "folder";

const CHOICE_LABELS: Record<ScopeChoice, MessageKey> = {
  all: "search.all",
  account: "search.account",
  folder: "search.folder",
};

/** Which scope choices make sense where the user is looking. */
export function availableChoices(location: MailLocation, folder: Folder | undefined): ScopeChoice[] {
  if (location.kind !== "folder") return ["all"];
  if (location.folderId === ALL_INBOXES_ID) return ["all", "folder"];
  return folder ? ["all", "account", "folder"] : ["all"];
}

export function scopeFor(choice: ScopeChoice, location: MailLocation, folder: Folder | undefined): SearchScope {
  if (choice === "account" && folder) return { kind: "account", accountId: folder.accountId };
  if (choice === "folder" && location.kind === "folder") return { kind: "folder", folderId: location.folderId };
  return { kind: "all" };
}

interface SearchState {
  hits: SearchHit[];
  isBusy: boolean;
}

/** Results while typing, a moment after the last key. */
export function useSearch(query: string, scope: SearchScope, onError: (e: unknown) => void): SearchState {
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [isBusy, setIsBusy] = useState(false);
  const scopeKey = JSON.stringify(scope);

  useEffect(() => {
    const q = query.trim();
    if (q === "") {
      setHits([]);
      setIsBusy(false);
      return;
    }
    let alive = true;
    setIsBusy(true);
    const timer = setTimeout(() => {
      api
        .search(q, JSON.parse(scopeKey) as SearchScope, SEARCH_LIMIT)
        .then((found) => {
          if (alive) setHits([...found].sort((a, b) => b.message.date.localeCompare(a.message.date)));
        })
        .catch((e: unknown) => {
          if (alive) onError(e);
        })
        .finally(() => {
          if (alive) setIsBusy(false);
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [query, scopeKey, onError]);

  return { hits, isBusy };
}

interface SearchBarProps {
  query: string;
  choice: ScopeChoice;
  choices: readonly ScopeChoice[];
  onQuery: (query: string) => void;
  onChoice: (choice: ScopeChoice) => void;
}

/** One box, "Zoeken", with a plain choice of where to look. */
export function SearchBar({ query, choice, choices, onQuery, onChoice }: SearchBarProps) {
  return (
    <div className="search-bar" role="search">
      <label htmlFor="search-box" className="search-label">
        {t("search.label")}
      </label>
      <input
        id="search-box"
        type="search"
        value={query}
        placeholder={t("search.placeholder")}
        onChange={(e) => onQuery(e.target.value)}
      />
      {query !== "" && (
        <button type="button" onClick={() => onQuery("")}>
          {t("search.clear")}
        </button>
      )}
      <div className="scope-choice" role="group" aria-label={t("search.where")}>
        {choices.map((c) => (
          <button
            key={c}
            type="button"
            className={`scope-button${c === choice ? " current" : ""}`}
            aria-pressed={c === choice}
            onClick={() => onChoice(c)}
          >
            {t(CHOICE_LABELS[c])}
          </button>
        ))}
      </div>
    </div>
  );
}
