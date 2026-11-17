# outlook-pst 1.2.0, patched for Vorige Eeuw Mail

A copy of `outlook-pst` 1.2.0 (MIT, https://github.com/microsoft/outlook-pst-rs) from crates.io,
used through `[patch.crates-io]` in the workspace `Cargo.toml`. Every change is marked
`vem patch` in the source. Each one only makes the reader accept a file it refused; nothing it
already read changes.

Found on 17 November 2026 by reading four public PST files (see `docs/PROGRESS.md`):

1. `src/ltp/heap.rs`, `HeapNodePageMap::new`: the free count (`cFree`) of a heap page is no
   longer checked. `enron.pst` from pst-extractor keeps 0 there although items were freed, and
   every one of its 71 messages failed with `InvalidHeapPageFreeCount(0)`. `readpst` reads them.
2. `src/messaging/message.rs`, the shared message reader: a message without a sub-node tree (no
   recipient or attachment table, such as a distribution list) opens with empty tables instead
   of failing with `MessageSubNodeTreeNotFound`.
3. `src/messaging/attachment.rs`: an attachment without `PidTagAttachMethod` is read as
   `afByValue`, the MS-OXCMSG default, instead of failing with `AttachmentMethodNotFound`
   (four attachments in Aspose's sample `Outlook.pst`).

When a new `outlook-pst` release fixes these, drop this folder and the `[patch]` entry.
