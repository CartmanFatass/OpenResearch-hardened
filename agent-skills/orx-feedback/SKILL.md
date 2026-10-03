---
name: orx-feedback
description: "Report product feedback about OpenResearch itself with `orx feedback`. Use when the user expresses frustration with an OpenResearch feature or bug, says a feature would be nice to have, or you hit a meaningful limitation or bug in the orx CLI, the agent harness, or the app. Not for research results, the user's own code, or minor nits."
---

# Report product feedback

`orx feedback` sends a report to the OpenResearch team through the configured
OpenResearch API. Reports may contain command inputs, error text and quoted user
words. Keep the bar high: a few precise reports are worth more than many vague
ones. Obtain explicit user consent to the report and its destination before
running the command. Enabling analytics does not grant consent to send reports.

## When to prepare a report

Consider a report only when one of these holds:

- The user explicitly shows frustration with an OpenResearch feature or bug.
- The user explicitly says a feature would be nice to have.
- You hit a meaningful limitation or bug in the `orx` CLI, the agent harness,
  or the app, such as a command that fails, hangs, or cannot express what the
  task needs.

Do not file for minor nits, for problems in the user's own code or
environment, or for a limitation you already reported earlier in this
session. File at most one report per turn.

## Get consent

Show the user the proposed kind, summary, details and optional quote, and identify
who receives them: the OpenResearch team at the configured API destination. Ask
whether to send that report, then wait for an affirmative answer. If the API
points somewhere else, disclose that actual destination before asking. Never
send merely because the user expressed frustration, requested a feature, or was
notified that you intended to send. An explicit user request to send an already
specified report to that destination is sufficient; do not ask again.

These are agent behavior instructions, not a technical security boundary. The
CLI and each harness's permission mode may permit network calls without a card.
Never rely on a permission prompt to obtain consent on your behalf.

## How to file

Run it as one line, with every value in single quotes:

```bash
orx feedback --kind bug --summary 'one line, at most 200 characters' --details 'failing input, command, error, expected result, and workaround' --quote 'the user words, optional'
```

`--kind` is `bug`, `feature_request`, or `frustration`. Keep each value on one
line and free of backticks, `$`, `<`, `>`, `|`, `;`, and `&` — the characters
that would end the single-quoted argument or start a substitution — so the
shell passes each value through as plain text. Replace only those characters
with bracketed names, such as `[ampersand]`, in commands, errors, and public
URLs. State that the bracketed names represent literal characters so inputs
can be restored. Keep the rest verbatim. Write an apostrophe as `'\''`.

Make a bug report reproducible on its own. Include as much relevant detail as
possible: the actual non-sensitive input, command and flags, error text as
above, expected and actual behavior, environment and version, and any
workaround. Preserve exact public inputs when they matter to reproduction. If
a needed detail is sensitive, redact only that part and say what was withheld;
if it is unavailable, say what is missing. Keep `--details` under 4000
characters and `--quote` under 1000.

## Protect the user's research

Leave out secrets, credentials, tokens, personal data, private paths, and
unpublished or proprietary research details. Sanitize sensitive parts of
commands and errors, but keep all relevant non-sensitive details, including
public inputs. Rephrase `--quote` only as needed to remove sensitive details.

## Tell the user

After running the approved command, report the actual outcome. Say it was sent
only when the CLI or service explicitly confirms submission. Exit code zero
alone is insufficient: versions of the CLI may return success without sending
when telemetry or the build channel disables feedback. If feedback is disabled,
say the report was not sent; never enable telemetry to get around that gate. If
the result is uncertain, say submission is unconfirmed and do not retry blindly.
Include the report kind and gist when confirming a verified submission.

If a value is rejected as invalid or too long, revise the draft and ask the user
to approve the changed report before trying again. For other failures, stop and
tell the user the report could not be confirmed as sent.
