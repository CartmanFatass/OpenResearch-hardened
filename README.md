<div align="center">

<h1><img src=".github/readme-assets/openresearch.svg" alt="" width="36" /> OpenResearch</h1>

**The local-first workspace for research agents and autoresearch.**

<p>Turn <img src=".github/readme-assets/claude.svg" alt="" width="16" height="16" align="texttop" /> Claude Code,
<img src=".github/readme-assets/codex.svg" alt="" width="16" height="16" align="texttop" /> Codex,
<img src=".github/readme-assets/opencode.svg" alt="" width="16" height="16" align="texttop" /> OpenCode,
<img src=".github/readme-assets/cursor.svg" alt="" width="16" height="16" align="texttop" /> Cursor, or Google Antigravity into research agents that can review
literature, develop hypotheses, run experiments, and produce research artifacts.</p>

<p><a href="#get-started">Build the hardened fork from source</a> · <a href="SECURITY_FIXES.md">Security changes and limitations</a></p>

</div>

> [!NOTE]
> **Security-hardened fork** of [alphaXiv/OpenResearch](https://github.com/alphaXiv/OpenResearch)
> (MIT, upstream © its authors — see `LICENSE`). Defaults here are the conservative side of
> upstream's: telemetry and auto-update are opt-in, installers are digest-verified, remote
> text is framed as untrusted, and implicit HuggingFace-token forwarding is restricted. Explicitly synced environment variables still reach every selected compute backend. See
> [SECURITY_FIXES.md](SECURITY_FIXES.md) for every change and the remaining known risks.

## Get started

Build **this repository** to obtain the hardening. The installers at
`openresearch.sh` and the binaries published by `alphaXiv/OpenResearch` are
upstream products and **do not include this fork's changes**. Do not use them
to install or update the hardened fork.

With Git and a current stable Rust toolchain installed:

```sh
git clone https://github.com/CartmanFatass/OpenResearch-hardened.git
cd OpenResearch-hardened
cargo build --release --locked
./target/release/orx up
```

On Windows, use `target\release\orx.exe up` after the build. On macOS and Linux,
the command above starts the local dashboard at `http://127.0.0.1:4791`.
The committed UI is embedded by the Rust build. See [AGENTS.md](AGENTS.md) for
UI development and validation commands.

Updates in this fork resolve only against this fork's release repository.
Until compatible fork releases are published, rebuild a reviewed revision
from source; there is no fallback to upstream. Source installs are not
installer-managed, so `orx update` does not replace them. The inherited
release-signing/official-build workflow requires separate maintainer setup;
this project does not claim upstream's code-signing or notarization identity.

Telemetry is disabled in source builds. Agent execution is still highly
autonomous by default; review the [remaining risks](SECURITY_FIXES.md#known-remaining-risks-deliberately-not-changed)
before supplying credentials or running untrusted research content.

[Connect a local model](docs/local-models.md) to use LM Studio, oMLX, Ollama,
or a custom endpoint with OpenCode. The platform-specific documents below
retain upstream packaging details and are not proof that hardened installers
are available.

## Built for research agents

| | OpenResearch gives you |
|---|---|
| **Parallel exploration** | Give each research direction an independent agent session and isolated git worktree. |
| **Reproducible experiments** | Track variants in a git-native experiment tree; every run receives an immutable archive of its recorded commit. |
| **Evidence in context** | Keep logs, diffs, files, results, and artifacts tied to the work that produced them. |
| **Your choice of agent** | Use Claude Code, Codex, OpenCode, Cursor, or Google Antigravity, with the harness and model selected per session. |
| **Your choice of compute** | Run locally, on your own infrastructure, or with managed OpenResearch compute. |
| **Local ownership** | Keep projects, conversations, experiments, runs, logs, code, and artifacts on your machine. |

### Autoresearch

OpenResearch can run the full loop autonomously: propose an idea, change the
code, launch an experiment, inspect the evidence, and decide what to try next.
Multiple agents can explore different directions in parallel while the
experiment tree preserves their lineage.

## Run anywhere

The same committed source snapshot can run locally, over SSH, or on Slurm,
Kubernetes, Ray, Hugging Face Jobs, Modal, Tinker, and managed OpenResearch compute.
Publishing the repository is not required.

Run the workspace next to remote GPUs while using the browser on your laptop:

```sh
orx up --remote user@host
```

SSH config aliases and custom ports are supported. The remote service binds to
loopback and has no application-level authentication, so other users on that
host can reach it.

## CLI and agent integration

Install the OpenResearch skill into supported coding agents:

```sh
orx install-skills
```

Common commands:

```sh
orx projects
orx project view <project-id>
orx runs <project-id>
orx logs <run-id>
orx exp run <experiment-id>
orx discover keyword <query>
orx paper <arxiv-id-or-doi>
```

Run `orx --help` or `orx <command> --help` for the complete interface.

## Local by default

OpenResearch runs on `127.0.0.1` with a local SQLite store. Creating a project
or launching a run does not publish your code. An
[openresearch.sh](https://openresearch.sh) account is only used for
service-owned capabilities such as organizations and managed compute.

## Usage analytics

Analytics are off by default. Once you opt in with `orx telemetry on`,
official release builds send coarse usage events tied to a random installation
ID. They do not include code, prompts, file contents or paths, repository
names, tokens, emails, or project and experiment identifiers.

```sh
orx telemetry status
orx telemetry on
orx telemetry off
orx <command> --no-telemetry
```

Source and development builds do not send analytics.

Coding agents may also file product feedback with `orx feedback` when you hit
a bug, wish for a feature, or get frustrated with OpenResearch. Each report is
a short description of the workflow problem. Bug reports include as much detail
as possible to reproduce a failure while omitting sensitive information. Like
analytics, reports are sent only from official release builds.
They are linked to your account when you are logged in and only sent while
analytics are enabled. The `--no-telemetry` flag covers only the command it is
passed to.
