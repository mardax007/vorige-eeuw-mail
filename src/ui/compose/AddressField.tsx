import { useEffect, useRef, useState, type Ref } from "react";
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
  /** A short explanation under the box, read out with it. */
  hint?: string;
  inputRef?: Ref<HTMLInputElement>;
}

function suggestionsFrom(contacts: readonly Contact[], typed: string): Addr[] {
  const q = typed.toLowerCase();
  return contacts
    .flatMap((c) => c.emails.map((email) => ({ name: c.name, email })))
    .filter((a) => a.name.toLowerCase().includes(q) || a.email.toLowerCase().includes(q))
    // An address typed in full needs no suggestion of itself.
    .filter((a) => a.email.toLowerCase() !== q)
    .slice(0, MAX_SUGGESTIONS);
}

/** An address box that suggests contacts while typing. */
export function AddressField({ id, label, value, onChange, problem = null, hint, inputRef }: AddressFieldProps) {
  const [suggestions, setSuggestions] = useState<Addr[]>([]);
  const rowRef = useRef<HTMLDivElement>(null);
  const typed = lastToken(value);
  const problemId = `${id}-problem`;
  const hintId = `${id}-hint`;
  const describedBy = [hint ? hintId : null, problem ? problemId : null].filter(Boolean).join(" ") || undefined;

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
    // Back in the box, ready for the next address.
    requestAnimationFrame(() => document.getElementById(id)?.focus());
  };

  return (
    <div
      className="form-row"
      ref={rowRef}
      onBlur={(e) => {
        // Leaving the box and its suggestions closes them; moving between the two does not.
        if (!rowRef.current?.contains(e.relatedTarget as Node | null)) setSuggestions([]);
      }}
    >
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        ref={inputRef}
        type="text"
        inputMode="email"
        autoComplete="off"
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        value={value}
        aria-invalid={problem ? true : undefined}
        aria-describedby={describedBy}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && suggestions.length > 0) {
            e.stopPropagation();
            setSuggestions([]);
          }
        }}
      />
      {hint && (
        <p id={hintId} className="hint">
          {hint}
        </p>
      )}
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
