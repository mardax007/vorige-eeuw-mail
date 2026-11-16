import { useEffect, useState, type Ref } from "react";
import { api } from "../../api/client";
import type { Addr, Contact } from "../../api/types";
import { t } from "../../i18n";
import { formatAddr } from "../format";
import { lastToken, replaceLastToken } from "./address";

export const SUGGEST_DEBOUNCE_MS = 150;
const MAX_SUGGESTIONS = 6;

interface AddressFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** What is wrong with this field, shown right under it. */
  problem?: string | null;
  inputRef?: Ref<HTMLInputElement>;
}

function suggestionsFrom(contacts: readonly Contact[], typed: string): Addr[] {
  const q = typed.toLowerCase();
  return contacts
    .flatMap((c) => c.emails.map((email) => ({ name: c.name, email })))
    .filter((a) => a.name.toLowerCase().includes(q) || a.email.toLowerCase().includes(q))
    .slice(0, MAX_SUGGESTIONS);
}

/** An address box that suggests contacts while typing. */
export function AddressField({ id, label, value, onChange, problem = null, inputRef }: AddressFieldProps) {
  const [suggestions, setSuggestions] = useState<Addr[]>([]);
  const typed = lastToken(value);
  const problemId = `${id}-problem`;

  useEffect(() => {
    if (typed.length < 2) {
      setSuggestions([]);
      return;
    }
    let alive = true;
    const timer = setTimeout(() => {
      api
        .listContacts(typed)
        .then((contacts) => {
          if (alive) setSuggestions(suggestionsFrom(contacts, typed));
        })
        // Suggestions are an extra; typing the address by hand always works.
        .catch(() => {
          if (alive) setSuggestions([]);
        });
    }, SUGGEST_DEBOUNCE_MS);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [typed]);

  const pick = (addr: Addr) => {
    onChange(replaceLastToken(value, addr));
    setSuggestions([]);
  };

  return (
    <div className="form-row">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        ref={inputRef}
        type="text"
        autoComplete="off"
        value={value}
        aria-invalid={problem ? true : undefined}
        aria-describedby={problem ? problemId : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {problem && (
        <p id={problemId} className="form-problem" role="alert">
          {problem}
        </p>
      )}
      {suggestions.length > 0 && (
        <ul className="suggestions" aria-label={t("compose.suggestions")}>
          {suggestions.map((a) => (
            <li key={a.email}>
              <button type="button" onClick={() => pick(a)}>
                {formatAddr(a)}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
