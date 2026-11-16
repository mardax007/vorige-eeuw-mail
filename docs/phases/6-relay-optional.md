# Phase 6 — Import over the internet (dropped)

**Dropped on 16 November 2026 (decision 12): Martijn chose no import over the internet.** Kept
only as a record of what was considered. Do not build it.

**Build this only if Martijn decides a USB stick is not enough** (decision 12). It is the only part
of the plan that needs a service of ours.

## What it is

The other computer encrypts the mail and uploads it to a relay; this app downloads and decrypts it.
On a current computer the sender is this same app in "send" mode. On an old PC it is a small helper
program.

## Requirements

- **A relay that cannot read the mail.** Encrypt on the sending computer. Do not derive the key
  from a short pairing code: that is guessable. Use a long key or a proper pairing protocol.
- **Delete each upload** after pickup, or after a fixed number of days.
- **A helper that still runs on old PCs.** Go builds a single file with no installer. Go 1.10 is the
  last version for Windows XP and Vista, Go 1.20 the last for Windows 7 and 8; newer versions need
  Windows 10. That means two or three builds of one small uploader.
- **A connection old machines can make.** The Go helper brings its own TLS 1.2, so the relay keeps
  normal TLS settings. Old Windows lacks current root certificates: build the relay's roots into the
  helper and test on a real XP machine. Do not lower the relay's minimum TLS version; Cloudflare's
  per-hostname setting is a paid feature anyway.
- **Big files.** Upload in chunks under Cloudflare's 100 MB request cap and resume after a dropped
  connection.
- **Storage.** Cloudflare R2's free tier holds 10 GB, which is smaller than some mail stores, and
  asks for a payment card before the first bucket.

## Before building

Write a one-page design and get Martijn's go-ahead: where the relay runs, how pairing works, how
long uploads live, what it costs.
