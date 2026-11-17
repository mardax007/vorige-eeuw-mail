use rusqlite::{OptionalExtension, params};

use super::{Store, new_id};
use crate::error::{Code, Result, VemError};
use crate::model::{MessageSummary, Tag};

/// Fixed palette keys the screens know (src/ui/tags.ts).
pub const TAG_COLORS: &[&str] = &["blue", "green", "yellow", "orange", "red", "purple", "grey"];

/// The IMAP keyword for a tag name: an atom without spaces or specials (RFC 3501 flag-keyword).
pub fn keyword_for(name: &str) -> String {
    let mut out = String::from("$vem_");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            // Case kept (docs/sync.md): another device shows "Familie", not "familie".
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    if out == "$vem_" || out.chars().all(|c| c == '_' || c == '$') {
        out.push_str(&new_id()[..8]);
    }
    out
}

impl Store {
    pub fn tags(&self) -> Result<Vec<Tag>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, name, color FROM tags ORDER BY name COLLATE NOCASE")?;
        let rows = st
            .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn create_tag(&self, name: &str, color: &str) -> Result<Tag> {
        let name = name.trim();
        if name.is_empty() {
            return Err(VemError::new(Code::AppCmd01).with_detail("empty tag"));
        }
        if let Some(t) = self.tags()?.into_iter().find(|t| t.name.eq_ignore_ascii_case(name)) {
            return Ok(t);
        }
        let color = if TAG_COLORS.contains(&color) { color } else { TAG_COLORS[0] };
        let id = new_id();
        let mut keyword = keyword_for(name);
        if self.tag_for_keyword(&keyword)?.is_some() {
            keyword = format!("{keyword}_{}", &id[..6]);
        }
        self.db().execute(
            "INSERT INTO tags (id, name, color, keyword) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, color, keyword],
        )?;
        Ok(Tag { id, name: name.into(), color: color.into() })
    }

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        self.db().execute("DELETE FROM tags WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn tag_keyword(&self, id: &str) -> Result<String> {
        self.db()
            .query_row("SELECT keyword FROM tags WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("tag"))
    }

    pub fn tag_for_keyword(&self, keyword: &str) -> Result<Option<Tag>> {
        Ok(self
            .db()
            .query_row(
                "SELECT id, name, color FROM tags WHERE keyword = ?1 COLLATE NOCASE",
                [keyword],
                |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }),
            )
            .optional()?)
    }

    /// A tag that came from the server (a keyword set elsewhere, or a Gmail label).
    pub fn tag_from_server(&self, keyword: &str, shown_name: &str) -> Result<Tag> {
        if let Some(t) = self.tag_for_keyword(keyword)? {
            return Ok(t);
        }
        if let Some(t) = self.tags()?.into_iter().find(|t| t.name.eq_ignore_ascii_case(shown_name)) {
            return Ok(t);
        }
        let id = new_id();
        let color = TAG_COLORS[self.tags()?.len() % TAG_COLORS.len()];
        self.db().execute(
            "INSERT INTO tags (id, name, color, keyword) VALUES (?1, ?2, ?3, ?4)",
            params![id, shown_name, color, keyword],
        )?;
        Ok(Tag { id, name: shown_name.into(), color: color.into() })
    }

    pub fn set_message_tag(&self, message_id: i64, tag_id: &str, on: bool) -> Result<()> {
        let db = self.db();
        if on {
            db.execute(
                "INSERT OR IGNORE INTO message_tags (message_id, tag_id) VALUES (?1, ?2)",
                params![message_id, tag_id],
            )?;
        } else {
            db.execute("DELETE FROM message_tags WHERE message_id = ?1 AND tag_id = ?2", params![message_id, tag_id])?;
        }
        Ok(())
    }

    /// Replace the tags of a message with those the server reports.
    pub fn set_message_tags(&self, message_id: i64, tag_ids: &[String]) -> Result<()> {
        let db = self.db();
        db.execute("DELETE FROM message_tags WHERE message_id = ?1", [message_id])?;
        for t in tag_ids {
            db.execute("INSERT OR IGNORE INTO message_tags (message_id, tag_id) VALUES (?1, ?2)", params![message_id, t])?;
        }
        Ok(())
    }

    /// Messages with a tag, across all accounts (docs/screens.md, Tags).
    pub fn messages_with_tag(&self, tag_id: &str, limit: u32) -> Result<Vec<MessageSummary>> {
        let tag = tag_id.to_string();
        self.summaries_where(
            "m.hidden = 0 AND m.id IN (SELECT message_id FROM message_tags WHERE tag_id = ?1)
             ORDER BY m.date DESC LIMIT ?2",
            &[&tag, &limit],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NewMessage;
    use crate::store::messages::tests::{mail, seed};
    use crate::store::testutil::store;

    #[test]
    fn keywords_are_atoms() {
        assert_eq!(keyword_for("Rekeningen 2026!"), "$vem_Rekeningen_2026_");
        assert!(keyword_for("€€").starts_with("$vem_"));
        assert!(!keyword_for("a b").contains(' '));
    }

    #[test]
    fn tag_messages_and_search_by_tag() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let t = s.create_tag("Familie", "green").unwrap();
        assert_eq!(s.create_tag("familie", "red").unwrap().id, t.id);
        let id = s
            .insert_message(&NewMessage { account_id: acc, folder_id: inbox, uid: Some(1), raw: mail("x", "p@x.nl", "y", "Mon, 16 Nov 2026 10:00:00 +0100"), has_body: true, ..Default::default() })
            .unwrap();
        s.set_message_tag(id, &t.id, true).unwrap();
        assert_eq!(s.messages_with_tag(&t.id, 10).unwrap()[0].tag_ids, vec![t.id.clone()]);
        assert_eq!(s.search("Famil", &crate::model::SearchScope::All, 10).unwrap().len(), 1);
        s.set_message_tag(id, &t.id, false).unwrap();
        assert!(s.messages_with_tag(&t.id, 10).unwrap().is_empty());
        let g = s.tag_from_server("Werk", "Werk").unwrap();
        assert_eq!(s.tag_for_keyword("werk").unwrap().unwrap().id, g.id);
    }
}
