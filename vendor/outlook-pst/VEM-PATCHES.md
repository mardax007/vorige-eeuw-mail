# outlook-pst 1.2.0, patched for Vorige Eeuw Mail

A copy of `outlook-pst` 1.2.0 (MIT, https://github.com/microsoft/outlook-pst-rs) from crates.io,
used through `[patch.crates-io]` in the workspace `Cargo.toml`. Every change is marked
`vem patch` in the source. Each one only makes the reader accept a file it refused; nothing it
already read changes.

Found on 17 November 2026 by importing four public PST files (see `docs/PROGRESS.md`): the Enron
sample from pst-extractor, Aspose's `Outlook.pst`, and java-libpst's `dist-list.pst` and
`passworded.pst`. Before these changes the normal reader lost most real messages; after them it
imports all four whole, and its attachment names match what `readpst` writes.

1. **`src/ltp/prop_type.rs`, `TryFrom<u16> for PropertyType`.** `0x000D` (`PtypObject`) was
   missing from the match although the rest of the crate already reads that type. A property-tree
   leaf that held one (an attached file or picture carries `PidTagAttachDataObject`) stopped
   there with `InvalidPropertyType(13)` and every property after it was lost, so the message or
   attachment failed. Added the arm.

2. **`src/ltp/heap.rs`, `HeapNodePageMap::new`.** The stored free count (`cFree`) of a heap page
   is no longer checked. `enron.pst` keeps `0` there although items were freed; the page map it
   builds is still right, but the check failed every one of its 71 messages with
   `InvalidHeapPageFreeCount(0)`.

3. **`src/messaging/message.rs`, `UnicodeMessage::read` / `AnsiMessage::read` (shared body).** A
   message with no sub-node tree — no recipient or attachment table, such as a distribution-list
   entry — now opens with empty tables instead of failing with `MessageSubNodeTreeNotFound`. Two
   edits: the sub-node is read as `Option`, and the sub-node map is empty when there is none.

4. **`src/messaging/attachment.rs`, `AttachmentInner::read` — method default.** An attachment
   without `PidTagAttachMethod` is read as `afByValue` (data in `PidTagAttachDataBinary`),
   MS-OXCMSG's default, instead of failing with `AttachmentMethodNotFound`.

5. **`src/messaging/attachment.rs`, `AttachmentInner::read` — sub-node lookup.** The node of an
   attached message or OLE storage (an `afEmbeddedMessage` / `afStorage` attachment) lives in the
   attachment's own sub-node tree ([MS-PST] 2.4.6.2), not in the parent message's. The parent's
   tree is now tried only as a fallback, which fixes `AttachmentSubNodeNotFound` on the embedded
   `message 1.eml` / `message 2.eml` attachments in Aspose's sample.

6. **`src/messaging/attachment.rs`, `AttachmentInner::read` — deferred embedded read.** Reading an
   embedded message inside `AttachmentInner::read` locked the PST file a second time while the
   first lock was still held, which dead-locked. The embedded node is now kept and read after the
   lock is released.

When a new `outlook-pst` release fixes these upstream, drop this folder and the `[patch]` entry.
