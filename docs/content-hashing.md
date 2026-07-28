# Content hashing: making "Changed" mean something

Status: implemented (hash basis, normalization, algo versioning). Near-match
scoring and change diffing remain open — see "Second-order improvements".
Scope: `core/src/monitor.rs`, `core/src/extract.rs`, `core/src/vault.rs`

Measured on the 53-save vault: `Changed` went from **36 to 4** on a back-to-back
re-check, and the remaining 4 are genuinely dynamic pages (a HN front page, a
Cloudflare dashboard, a rotating icon gallery, a live settings page) — true
positives, not noise.

## The problem

`Changed` currently fires far too often. On a real vault of 53 saves, 36 were
flagged `Changed` — 68%. A status that two thirds of your library carries at
rest is not a signal, it's the background colour. The user learns to ignore the
badge, and the one link that genuinely changed is indistinguishable from the 35
that didn't.

The cause is in `probe()`:

```rust
let hash = hex_sha256(&body);
```

`body` is the raw response bytes — the entire HTML document, capped at
`MAX_BODY_BYTES`. Every one of the following flips that hash without a single
word of the page changing:

- **Counters and timestamps.** A GitHub repo page embeds its star count, fork
  count, open-issue count, and "updated 3 hours ago" strings. MuseScore and
  MonitorControl in the sample vault are both GitHub pages, both flagged
  `Changed`. They will be flagged `Changed` on essentially every check forever.
- **CSRF tokens and nonces.** Emitted fresh per request by most frameworks.
- **Build hashes in asset URLs.** `/assets/index-a3f9c2.js` changes on every
  deploy, including deploys that touch nothing the reader sees.
- **Session and request IDs.** `data-request-id`, trace headers echoed into the
  document, A/B test bucket assignments.
- **Analytics and ad payloads.** Inline `<script>` blocks with per-impression
  identifiers. These alone guarantee a hash change per request on many sites.
- **Rotating boilerplate.** "Trending now" sidebars, related-post widgets,
  rotating hero images, cookie-banner variants.
- **Server-side rendering nondeterminism.** Attribute ordering, whitespace, and
  hydration payloads that differ between backends behind a load balancer.

None of these are what a bookmark manager's user means by "this page changed."

## The fix

Hash the extracted readable text, not the raw body.

The machinery already exists and already runs. In `probe()`, for every HTML
response, we compute:

```rust
let (og_image, archive_text) = if is_html {
    let html = String::from_utf8_lossy(&body);
    (
        extract_og_image(&html, &final_url),
        Some(crate::extract::extract_readable_text(&html)),
    )
} else {
    (None, None)
};
```

`extract_readable_text` already strips `script`, `style`, `noscript`, `svg`,
`template` and `head`; already prefers the `<article>` / `<main>` subtree over
page chrome; already decodes entities and collapses whitespace. That is exactly
the boilerplate-stripping a content hash needs, and we are already paying for
it on the archive path.

So the change is to hash that string instead:

```rust
let hash = match &archive_text {
    Some(text) if !text.is_empty() => hex_sha256(text.as_bytes()),
    // Non-HTML (PDFs, images, JSON) and pages that extract to nothing:
    // fall back to the raw body, which is the right basis for those anyway.
    _ => hex_sha256(&body),
};
```

Near-zero added cost, and it eliminates every noise source in the list above
that lives outside the article subtree — which is most of them.

### Why this is the right cut

Hashing the readable text means `Changed` answers the question the user is
actually asking: *did the words on this page change?* Not "did the server
produce different bytes," which is almost always yes and almost never
interesting.

It also composes correctly with the rest of the feature. `archive_text` is
already stored for search. Hashing the same string means the hash and the
archive can never disagree about what was seen — the hash is a fingerprint of
exactly the content we kept.

## Second-order improvements

These are worth doing but are strictly less important than the change above.

### 1. Normalize before hashing

`extract_readable_text` collapses whitespace but preserves case and
punctuation, which is right for search and slightly too strict for change
detection. A dedicated normalization step before hashing would absorb another
tier of noise:

- Lowercase.
- Strip digits that stand alone as tokens, or at minimum normalize sequences
  matching common counter shapes (`169k`, `3.2k`, `12,481`). A star count
  ticking from 169k to 170k is not a content change.
- Normalize relative-time phrases (`3 hours ago`, `last week`, `2 minutes
  ago`) to a fixed sentinel. These appear on virtually every repo, forum, and
  news page and change continuously.

Keep this as a separate `fn normalize_for_hash(text: &str) -> String` in
`extract.rs` so the archive text stays verbatim and only the hash basis is
normalized.

### 2. Near-match instead of exact match

Even normalized, exact hashing is binary: one changed word reads the same as a
rewritten page. A similarity measure would let the UI distinguish "a typo was
fixed" from "this article was replaced."

Cheapest useful version: store a SimHash or a sorted-shingle MinHash sketch
alongside the exact hash, and treat a Hamming/Jaccard distance under some
threshold as `Active`. This turns the threshold into a tunable rather than a
cliff, and enables a much better UI affordance — "changed slightly" vs
"substantially rewritten."

This is a real addition (new column, new dependency or ~80 lines of hashing
code) and should not block the primary fix.

### 3. Surface *what* changed

Once the hash is trustworthy, the natural next question from the user is "what
changed?" We already store `archive_text` from the previous check. Diffing the
old and new extracted text and showing a few changed lines would make `Changed`
actionable rather than merely informative. This is the feature that would
distinguish WebSave from every other bookmark manager, and it is only possible
once the hash stops crying wolf.

## A bug this surfaced: empty extraction on real sites

While validating the change against the live vault, GitHub repo pages kept
reporting `Changed` even after the switch. The cause was not hashing — it was
`extract_readable_text` returning `""` for those pages, which routed them to
the raw-body fallback and straight back into the old noisy behaviour.

`subtree()` picks the *first* `<article>` and the *last* `</article>`. GitHub
emits a small decorative `<article>` before the README, so the extractor
selected ~2 KB of markup containing no prose, flattened it to nothing, and
returned empty — never falling through to `<main>` or `<body>`.

The blast radius was wider than change detection: **every such page has been
archiving as empty text**, so archive search silently did not cover them.

Fixed by trying each candidate scope in order and keeping the first that
actually yields text, rather than committing to the first container that
merely exists.

## Migration

This is the part that needs care. Changing the hash basis invalidates every
stored `content_hash`. On the next check, every save would compare a
readable-text hash against a stored raw-body hash, mismatch, and flip to
`Changed` — briefly making the exact problem worse before it gets better.

Two options:

**A. Version the hash column.** Add `content_hash_algo INTEGER NOT NULL DEFAULT 0`
to `saves`. When the stored algo doesn't match the current one, treat the check
as a re-baseline: write the new hash, set status to `Active`, do not report
`Changed`. Self-healing, no user-visible blip, and the mechanism is reusable
the next time the basis changes (e.g. when normalization lands).

**B. One-shot reset.** A migration that blanks `content_hash` for all rows.
`probe()` already handles the empty case correctly — `!previous_hash.is_empty()`
guards the `Changed` branch, so a blank hash yields `Active` and re-baselines
on the next check. Simpler, but throws away the ability to distinguish
"never checked" from "checked under an old algorithm."

Prefer **A**. It is a few more lines and it means the next change to the hashing
strategy is a one-line constant bump rather than another migration.

## Test plan

`extract.rs` already has good tests to model these on.

- A page whose only diff is a counter (`169k` → `170k` stars) hashes equal
  after normalization.
- A page whose only diff is an inline analytics `<script>` payload hashes
  equal (already covered by the existing strip, worth asserting explicitly).
- A page whose only diff is a relative timestamp hashes equal after
  normalization.
- A page with one paragraph genuinely rewritten hashes differently.
- A non-HTML response (PDF) still falls back to body hashing and still detects
  a real change.
- A page that extracts to empty text falls back to body hashing rather than
  hashing the empty string — otherwise every content-free page collides with
  every other and no change is ever detected.

That last case is the one most likely to be missed: without the
`!text.is_empty()` guard, all pages that fail extraction share a single hash
and permanently report `Active`.

## Summary

The primary change is a handful of lines in `probe()` reusing extraction work
already being done, plus a hash-versioning column so the transition doesn't
produce a wave of false `Changed` flags. That alone should take the flagged
rate from ~68% to something in the low single digits. Normalization, near-match
scoring, and change diffing are worthwhile follow-ups that all get easier once
the hash basis is right.
