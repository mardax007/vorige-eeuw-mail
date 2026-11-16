//! Search on the local index at three levels: all mail, one mailbox, one folder (PLAN.md §2).

use super::Store;
use crate::error::Result;
use crate::model::{SearchHit, SearchScope};

/// Turn what the user typed into an FTS5 query: every word must occur, as a prefix, so results
/// come while typing. Quotes and operators the user types are treated as plain text.
pub fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split(|c: char| c.is_whitespace() || "\"*^():+-'`,.;!?".contains(c))
        .filter(|w| !w.is_empty())
        .take(12)
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect();
    if words.is_empty() { None } else { Some(words.join(" ")) }
}

impl Store {
    pub fn search(&self, input: &str, scope: &SearchScope, limit: u32) -> Result<Vec<SearchHit>> {
        let Some(q) = fts_query(input) else { return Ok(Vec::new()) };
        let tag_like = format!("{}%", input.trim().replace(['%', '_'], ""));
        let (scope_sql, scope_arg): (&str, Option<String>) = match scope {
            SearchScope::All => ("", None),
            SearchScope::Account { account_id } => (" AND m.account_id = ?4", Some(account_id.clone())),
            SearchScope::Folder { folder_id } => (" AND m.folder_id = ?4", Some(folder_id.clone())),
        };
        let filter = format!(
            "m.hidden = 0 AND (m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1)
                OR m.id IN (SELECT mt.message_id FROM message_tags mt JOIN tags t ON t.id = mt.tag_id
                            WHERE t.name LIKE ?2))
             {scope_sql}
             ORDER BY m.date DESC, m.id DESC LIMIT ?3"
        );
        let hits = match &scope_arg {
            Some(a) => self.summaries_where(&filter, &[&q, &tag_like, &limit, a])?,
            None => self.summaries_where(&filter, &[&q, &tag_like, &limit])?,
        };
        let names = self.place_names()?;
        Ok(hits
            .into_iter()
            .map(|m| {
                let (account_name, folder_name) = names
                    .get(&m.folder_id)
                    .cloned()
                    .unwrap_or_default();
                SearchHit { message: m, account_name, folder_name }
            })
            .collect())
    }

    /// folder id -> (account name, folder name), for naming where each hit lives.
    fn place_names(&self) -> Result<std::collections::HashMap<String, (String, String)>> {
        let db = self.db();
        let mut st = db.prepare(
            "SELECT f.id, a.display_name, f.name FROM folders f JOIN accounts a ON a.id = f.account_id",
        )?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, i64>(0)?.to_string(), (r.get(1)?, r.get(2)?))))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountKind, FolderRole};
    use crate::store::NewMessage;
    use crate::store::messages::tests::{mail, seed};
    use crate::store::testutil::store;

    #[test]
    fn query_building_is_safe() {
        assert_eq!(fts_query("  café  ber"), Some("\"café\"* \"ber\"*".into()));
        assert_eq!(fts_query("\"NEAR(a b)\" OR -x"), Some("\"NEAR\"* \"a\"* \"b\"* \"OR\"* \"x\"*".into()));
        assert_eq!(fts_query("  "), None);
    }

    #[test]
    fn three_levels_prefix_and_diacritics() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let other = s.upsert_folder(&acc, "Werk", None, None, true).unwrap();
        let b = s.add_account("B", "b@x.nl", AccountKind::Imap, None).unwrap();
        let inbox_b = s.upsert_folder(&b.id, "INBOX", None, Some(FolderRole::Inbox), true).unwrap();
        let add = |acc: &str, f: i64, subj: &str, body: &str| {
            s.insert_message(&NewMessage {
                account_id: acc.into(),
                folder_id: f,
                uid: None,
                raw: mail(subj, "Piet <piet@x.nl>", body, "Mon, 16 Nov 2026 10:00:00 +0100"),
                has_body: true,
                ..Default::default()
            })
            .unwrap()
        };
        add(&acc, inbox, "Factuur", "Het café is open");
        add(&acc, other, "Rekening", "cafe morgen");
        add(&b.id, inbox_b, "Nieuws", "Café de Zwaan");
        assert_eq!(s.search("cafe", &SearchScope::All, 50).unwrap().len(), 3);
        assert_eq!(s.search("caf", &SearchScope::Account { account_id: acc.clone() }, 50).unwrap().len(), 2);
        let f = s.search("café", &SearchScope::Folder { folder_id: other.to_string() }, 50).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].folder_name, "Werk");
        assert_eq!(f[0].account_name, "Jan");
        assert_eq!(s.search("piet fact", &SearchScope::All, 50).unwrap().len(), 1);
        assert!(s.search("zzz", &SearchScope::All, 50).unwrap().is_empty());
    }
}
