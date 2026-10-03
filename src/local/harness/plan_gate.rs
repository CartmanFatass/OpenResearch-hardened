//! Conservative plan-mode auto-approval for a small set of inspection commands.
//!
//! The classifier accepts only literal shell words, simple pipelines and batches.
//! Anything dynamic, unknown or potentially writing defers to the normal approval
//! flow. This is an approval shortcut, not an OS sandbox: the shell environment
//! and installed executables must still be trusted. In particular, Git commands
//! require approval because repository/user configuration can execute programs
//! even during a nominal read (fsmonitor, textconv, pagers and remote helpers).

use serde_json::{json, Value};

/// Top-level `orx` verbs that never change the project in any form (no
/// subcommand can turn them into a write). Kept in lockstep with `main.rs`'s `Command`
/// enum; `readonly_verbs_are_real_commands` guards against a rename.
#[cfg(test)]
const WHOLE_VERB_READS: &[&str] = &[
    "projects", "orgs", "runs", "logs", "discover", "paper", "skill", "version",
];
// `feedback` is deliberately absent: it POSTs over the network and follows the
// normal permission flow instead of being auto-approved by this plan gate.

/// Shell no-ops allowed as glue between read-only segments in a batch —
/// separators and labels the planning agent prints, e.g. `echo ====`. They take
/// arbitrary args but can't have a side effect here (redirection/substitution
/// are rejected before we get this far), so a program-name match is enough.
const READONLY_GLUE: &[&str] = &["echo", "true", ":"];

/// Stream tools with no supported file-output or command-execution mode.
/// `sort` and `uniq` are deliberately excluded: both accept an output file, and
/// `sort --compress-program` can execute a helper. Git is not auto-approved.
const PURE_CONSUMERS: &[&str] = &[
    "head", "tail", "grep", "egrep", "fgrep", "wc", "cat", "cut", "tr", "nl", "column",
];

/// Decide whether a `PreToolUse` hook payload describes a read-only command
/// that plan mode should let through. Returns the JSON to print on stdout (an
/// `allow`/`ask` decision) or `None` to stay silent and defer to plan mode's
/// default gating.
///
/// The payload shape is Claude Code's hook contract: `tool_name` names the tool
/// and `tool_input.command` carries the Bash command line.
///
/// ExitPlanMode gets an explicit `ask`: headless plan mode **self-approves**
/// the call otherwise ("User has approved exiting plan mode", nobody asked —
/// verified on claude 2.1.197), letting the model exit plan mode and edit
/// files without the user's say. `ask` routes it to the permission prompt tool
/// (the `orx mcp-gate` bridge), which surfaces the plan card and blocks until
/// the user answers; without a bridge configured, `ask` in headless denies —
/// still strictly better than self-approval.
pub fn decide(payload: &Value) -> Option<Value> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;

    if tool_name == "ExitPlanMode" {
        return Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "ask",
                "permissionDecisionReason":
                    "plan approval is the user's decision",
            }
        }));
    }

    // Only Bash calls carry a shell command to inspect; every other tool defers.
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .pointer("/tool_input/command")
        .and_then(Value::as_str)?;

    if !command_is_readonly(command) {
        return None;
    }

    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason":
                "read-only inspection is allowed during plan mode",
        }
    }))
}

/// Auto-approve only literal, individually read-only commands. The separator
/// split is intentionally conservative: separators inside quotes may defer a
/// safe command, but each resulting stage must have balanced quotes. The word
/// tokenizer removes literal quoting before option classification and rejects
/// shell expansion, so quotes cannot conceal a writing option.
pub fn command_is_readonly(command: &str) -> bool {
    // Bash does not split on all Unicode whitespace. Reject controls and
    // non-shell whitespace rather than interpreting a different argument list.
    if command
        .chars()
        .any(|c| (c.is_control() || c.is_whitespace()) && !matches!(c, ' ' | '\t'))
    {
        return false;
    }
    command
        .trim_matches([' ', '\t'])
        .split("&&")
        .flat_map(|part| part.split(';'))
        .all(is_readonly_segment)
}

/// True iff a single command segment (no `;`/`&&` separators) is a read-only
/// pipeline: a read-only producer optionally piped through pure consumers.
/// Empty/whitespace segments (from a leading/trailing/doubled separator) are
/// shell no-ops and allowed, matching the shell's own tolerance for `orx runs p-1;`.
fn is_readonly_segment(segment: &str) -> bool {
    let segment = segment.trim();
    if segment.is_empty() {
        return true;
    }

    let mut stages = segment.split('|');
    // `split` always yields at least one item.
    let first = stages.next().unwrap_or_default();
    if !is_readonly_producer(first) {
        return false;
    }
    // Every later stage must be a pure consumer. An empty stage means `||`,
    // `| |`, or a trailing pipe — never legitimate read-only batching.
    stages.all(|stage| match stage_tokens(stage) {
        Some(tokens) if !tokens.is_empty() => is_pure_consumer(&tokens),
        _ => false,
    })
}

/// Parse a deliberately small shell subset: literal words and single/double
/// quoting only. No expansion, escapes outside single quotes, comments, globs,
/// grouping or redirection. A standalone, unquoted `2>&1` is the sole exception.
/// This is not a general shell parser; unsupported syntax requires approval.
fn stage_tokens(stage: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = stage.chars().collect();
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(delimiter) = quote {
            if c == delimiter {
                quote = None;
            } else {
                // Single quotes are entirely literal. Double quotes still
                // expand these characters in Bash, so defer those forms.
                if delimiter == '"' && matches!(c, '$' | '`' | '\\') {
                    return None;
                }
                word.push(c);
            }
        } else if matches!(c, ' ' | '\t') {
            if started {
                tokens.push(std::mem::take(&mut word));
                started = false;
            }
        } else if !started
            && chars[i..].starts_with(&['2', '>', '&', '1'])
            && chars.get(i + 4).is_none_or(|c| matches!(c, ' ' | '\t'))
        {
            i += 4;
            continue;
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
            started = true;
        } else {
            if matches!(
                c,
                '$' | '`'
                    | '\\'
                    | '<'
                    | '>'
                    | '&'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '*'
                    | '?'
                    | '~'
                    | '#'
                    | '!'
            ) {
                return None;
            }
            word.push(c);
            started = true;
        }
        i += 1;
    }
    if quote.is_some() {
        return None;
    }
    if started {
        tokens.push(word);
    }
    Some(tokens)
}

/// True iff a pipeline's first stage is read-only: glue, a read-only `orx`
/// invocation, or a pure consumer (so `wc -l f`
/// or `head -50 f` stand alone too).
fn is_readonly_producer(stage: &str) -> bool {
    let Some(tokens) = stage_tokens(stage) else {
        return false;
    };
    let Some(program) = tokens.first().map(String::as_str) else {
        return false;
    };
    if READONLY_GLUE.contains(&program) {
        return true;
    }
    // A matching basename is not identity: /tmp/untrusted/orx could do anything.
    // The plain name is the app-provided PATH entry; explicit paths must name
    // this running executable, not merely end in /orx.
    if program == "orx"
        || crate::paths::spawnable_exe().is_ok_and(|path| path == std::path::Path::new(program))
    {
        return is_readonly_orx(&tokens);
    }
    is_pure_consumer(&tokens)
}

fn is_pure_consumer(tokens: &[String]) -> bool {
    tokens
        .first()
        .is_some_and(|program| PURE_CONSUMERS.contains(&program.as_str()))
}

/// Parse normalized literal argv using the actual CLI grammar. In particular,
/// `--s''et` is already `--set` here, and is classified as a write. Future
/// commands and invalid argument combinations fail closed.
fn is_readonly_orx(tokens: &[String]) -> bool {
    use crate::commands::compute::{ComputeCommand, InstructionsCommand, SshConfigCommand};
    use crate::{Command, ComputeArgs, ExpArgs, ExpCommand, ProjectArgs, ProjectCommand};
    use clap::Parser;

    let command = match crate::Cli::try_parse_from(tokens) {
        Ok(cli) => cli.command,
        Err(error) => {
            return matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
        }
    };
    // Bare orx is not reliably a help-only operation: platform launch logic
    // can open the desktop app. Explicit --help/--version returned above.
    matches!(
        command,
        Some(
            Command::Projects(_)
                | Command::Orgs(_)
                | Command::Runs(_)
                | Command::Logs(_)
                | Command::Discover(_)
                | Command::Paper(_)
                | Command::Skill(_)
                | Command::Version(_)
                | Command::Project(ProjectArgs {
                    command: ProjectCommand::View { .. }
                })
                | Command::Exp(ExpArgs {
                    command: ExpCommand::Status { .. }
                        | ExpCommand::Wait { .. }
                        | ExpCommand::Desc {
                            set: None,
                            stdin: false,
                            ..
                        }
                })
                | Command::Compute(ComputeArgs {
                    command: None
                        | Some(
                            ComputeCommand::Catalog(_)
                                | ComputeCommand::Status
                                | ComputeCommand::Show { .. }
                                | ComputeCommand::Instructions {
                                    command: InstructionsCommand::Show
                                }
                                | ComputeCommand::SshConfig {
                                    command: SshConfigCommand::Show
                                }
                        ),
                    ..
                })
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(command: &str) -> Value {
        json!({ "tool_name": "Bash", "tool_input": { "command": command } })
    }

    fn allowed(command: &str) -> bool {
        decide(&payload(command)).is_some()
    }

    #[test]
    fn stream_consumers_with_write_or_execution_modes_are_gated() {
        for command in [
            "sort -o /tmp/target /tmp/input",
            "sort --output=/tmp/target /tmp/input",
            "sort --compress-program=/tmp/helper /tmp/input",
            "echo payload | uniq - /tmp/target",
            "uniq /tmp/input /tmp/target",
        ] {
            assert!(!allowed(command), "should gate: {command}");
        }
    }

    #[test]
    fn shell_normalization_cannot_hide_write_flags() {
        for command in [
            "orx exp desc e-1 --s''et=changed",
            "orx exp desc e-1 --\"set\"=changed",
            r"orx exp desc e-1 --s\et=changed",
            "orx exp desc e-1 --st''din",
            "orx exp desc e-1 ${FLAGS}",
            "orx exp desc e-1 *",
            "orx exp desc e-1 $'--set=changed'",
            "orx exp desc e-1 # --set=changed",
            "orx exp desc e-1\r--set=changed",
            "orx exp desc e-1\u{00a0}--set=changed",
        ] {
            assert!(!allowed(command), "should gate: {command}");
        }
    }

    #[test]
    fn git_reads_with_configured_executables_require_approval() {
        // Even apparently plain reads can invoke fsmonitor, a pager, textconv,
        // external diffs or remote helpers from repository/user configuration.
        for command in [
            "git status",
            "git log",
            "git diff --ext-diff",
            "git show --textconv HEAD:file",
            "git remote show origin",
            "git log --out''put=/tmp/target",
            "git branch -uorigin/main",
            "git branch --edit-descrip",
            "/tmp/untrusted/git status",
        ] {
            assert!(!allowed(command), "should gate: {command}");
        }
    }

    #[test]
    fn bare_invocations_require_approval_but_explicit_help_does_not() {
        // main() may launch the desktop app for an absent subcommand.
        for command in ["orx", "orx --no-telemetry"] {
            assert!(!allowed(command), "should gate: {command}");
        }
        let executable = crate::paths::spawnable_exe().unwrap();
        assert!(!allowed(&format!("'{}'", executable.display())));
        for command in ["orx --help", "orx -h", "orx --no-telemetry --help"] {
            assert!(allowed(command), "should allow: {command}");
        }
    }

    #[test]
    fn executable_basename_does_not_establish_identity() {
        assert!(!allowed("/tmp/untrusted/orx runs p-1"));
        assert!(!allowed("./orx runs p-1"));
    }

    #[test]
    fn literal_quoted_arguments_are_normalized_and_safe_reads_survive() {
        for command in [
            "orx discover keyword 'attention mechanisms'",
            "orx discover keyword \"attention mechanisms\"",
            "orx exp desc 'e-1'",
            "orx projects --json",
            "orx runs p-1 2>&1 | head -5",
            "orx --help",
            "orx compute configure ssh --help",
            "echo ''",
        ] {
            assert!(allowed(command), "should allow: {command}");
        }
        let executable = crate::paths::spawnable_exe().unwrap();
        assert!(allowed(&format!("'{}' runs p-1", executable.display())));
    }

    #[test]
    fn literal_word_tokenizer_matches_shell_quote_removal() {
        assert_eq!(
            stage_tokens("orx exp desc 'e-1' --s''et=changed 2>&1"),
            Some(
                vec!["orx", "exp", "desc", "e-1", "--set=changed"]
                    .into_iter()
                    .map(String::from)
                    .collect()
            )
        );
        assert_eq!(
            stage_tokens("echo '' '2>&1'"),
            Some(
                vec!["echo", "", "2>&1"]
                    .into_iter()
                    .map(String::from)
                    .collect()
            )
        );
        assert!(allowed("orx discover keyword 'literal $dollar'"));
    }

    #[test]
    fn incomplete_or_dynamic_shell_syntax_defers() {
        for command in [
            "orx exp desc 'e-1",
            "orx exp desc \"e-1",
            "orx exp desc \"$EXPERIMENT\"",
            "orx exp desc $(echo e-1)",
            "orx exp desc `echo e-1`",
            "orx exp desc e-1 > /tmp/target",
            "orx exp desc e-1 2>&1suffix",
            "orx exp desc e-1 &",
            "orx exp desc e-1\norx projects",
            "orx projects || echo fallback",
            "orx projects |",
            "orx projects | | head",
            "2>&1",
        ] {
            assert!(!allowed(command), "should gate: {command}");
        }
    }

    #[test]
    fn compute_reads_are_allowed_but_setup_and_instruction_writes_are_gated() {
        for command in [
            "orx compute",
            "orx compute --gpu H100",
            "orx compute status --json",
            "orx compute show ssh",
            "orx compute instructions show --json",
            "orx compute configure ssh --help",
            "orx compute ssh-config show",
        ] {
            assert!(allowed(command), "{command}");
        }
        for command in [
            "orx compute default clear",
            "orx compute configure ssh --default-host lab",
            "orx compute connect ssh --host lab",
            "orx compute instructions path",
            r"orx compute instructions set --file my\ --help --expected-revision x",
            "orx compute default set slurm --flavor=${NOPE:+x --help y}",
            "orx compute configure slurm --partition gpu\r--help",
            "orx compute configure slurm --partition gpu\u{00a0}--help",
            "orx compute instructions set --file x --expected-revision y",
            "orx compute ssh-config set --file x --previous-file y",
        ] {
            assert!(!allowed(command), "{command}");
        }
    }

    #[test]
    fn whole_verb_reads_are_allowed() {
        for c in [
            "orx runs p-1",
            "orx logs r-123",
            "orx compute",
            "orx orgs",
            "orx discover keyword transformers",
            "orx discover embedding transformers --prioritize historical",
            "orx paper 2301.00001",
            "orx skill",
            "orx projects --json",
        ] {
            assert!(allowed(c), "should allow: {c}");
        }
    }

    #[test]
    fn read_only_subcommands_are_allowed() {
        assert!(allowed("orx project view p-1"));
        assert!(allowed("orx exp status e-1"));
        // The playbook's core orientation reads (view form).
        assert!(allowed("orx exp desc e-1"));
        assert!(allowed("orx exp wait e-1"));
    }

    #[test]
    fn write_subcommands_are_gated() {
        // Same verb, write subcommand → not allowed (plan mode gates it).
        assert!(!allowed("orx project edit p-1 --name x"));
        assert!(!allowed("orx exp run e-1"));
        assert!(!allowed("orx exp cancel e-1"));
        // `desc` becomes a write with --set/--stdin → gated.
        assert!(!allowed("orx exp desc e-1 --set \"found X\""));
        assert!(!allowed("orx exp desc e-1 --stdin"));
    }

    #[test]
    fn launch_and_write_verbs_are_gated() {
        for c in [
            "orx exp run e-1 --backend hf",
            "orx instance create --gpu a100",
            "orx create-experiment p-1 --title x --parent e-1",
            "orx login",
            "orx logout",
            "orx install-skills",
            "orx update",
            "orx delete database",
            "orx delete cli",
            "orx delete all",
            "orx up",
            "orx serve",
            // A network POST defers to the active permission mode instead of
            // being auto-approved by this classifier.
            "orx feedback --kind bug --summary 'x' --details 'y, then z'",
        ] {
            assert!(!allowed(c), "should gate: {c}");
        }
    }

    #[test]
    fn unknown_verb_is_gated() {
        // Allowlist-only: a future verb we haven't classified defaults to gated.
        assert!(!allowed("orx teleport e-1"));
    }

    #[test]
    fn non_orx_commands_defer() {
        assert!(!allowed("rm -rf /"));
        assert!(!allowed("git push"));
        assert!(!allowed("python train.py"));
        // A program that merely ends in text containing orx is not `orx`.
        assert!(!allowed("neworx runs p-1"));
    }

    #[test]
    fn dangerous_chaining_is_rejected() {
        // Sequencing is validated per-segment (see `readonly_sequences_are_allowed`),
        // but any segment that is a write, or any metacharacter a per-segment scan
        // can't reason about, gates the whole line. A read-only prefix must never
        // smuggle a second command through.
        assert!(!allowed("orx runs p-1; orx exp run e-1")); // write segment
        assert!(!allowed("orx runs p-1 && rm -rf /")); // non-orx segment
        assert!(!allowed(
            "orx runs p-1; echo hi; orx create-experiment p-1 --title x"
        )); // write in a batch
        assert!(!allowed("orx runs p-1 | tee /etc/passwd")); // pipe into a writer
        assert!(!allowed("orx logs r-1 > /tmp/x")); // redirection
        assert!(!allowed("orx logs \"$(rm -rf /)\"")); // command substitution
        assert!(!allowed("orx runs `whoami`")); // backtick substitution
        assert!(!allowed("orx runs &")); // background execution
        assert!(!allowed("orx runs & rm -rf /")); // single-& chaining
    }

    #[test]
    fn readonly_sequences_are_allowed() {
        // The planning agent batches several read-only lookups into one call,
        // punctuated by `echo` separators — the exact pattern plan mode was
        // failing to run. Each segment is independently read-only, so the whole
        // line is allowed.
        assert!(allowed("orx exp desc e-1; echo ====; orx exp desc e-2"));
        assert!(allowed("orx runs p-1 && orx logs r-1"));
        assert!(allowed("orx exp desc e-1 && orx project view p-1"));
        // Mixed `&&` and `;` in one line exercises the two-level split.
        assert!(allowed("orx runs p-1 && orx logs r-1; echo done"));
        // Harmless glue on its own, and shell-tolerated trailing/empty segments.
        assert!(allowed("echo hello"));
        assert!(allowed("orx runs p-1;"));
        assert!(allowed("orx runs p-1 ; ; orx logs r-1"));
        // A read/view form batched with its own write form still gates: the
        // `--set` segment is a write.
        assert!(!allowed("orx exp desc e-1; orx exp desc e-1 --set \"x\""));
    }

    #[test]
    fn readonly_pipelines_are_allowed() {
        // The other pattern plan mode kept failing on: reads piped through pure
        // consumers, with the customary stderr-merge.
        assert!(allowed("orx runs p-1 2>&1 | head -50"));
        assert!(allowed("orx logs r-1 2>&1 | grep -i error | tail -5"));
        assert!(allowed("orx runs r-1 2>&1"));
        assert!(!allowed("git log --oneline | head -20"));
        assert!(!allowed("git ls-tree -r HEAD | grep -i py"));
        assert!(!allowed("git show origin/b:mem2gen/orx_run.py | head -100"));
        // Consumers stand alone and pipe among themselves.
        assert!(allowed("wc -l README.md"));
        assert!(allowed("cat notes.md | grep TODO | wc -l"));
        // Pipelines and sequences compose.
        assert!(allowed("orx runs p-1 | head -5; echo ok && orx projects"));
        // No-space pipes parse the same way the shell parses them.
        assert!(allowed("orx runs p-1 2>&1|head -5"));
    }

    #[test]
    fn nonreadonly_pipelines_are_gated() {
        assert!(!allowed("orx runs p-1 | xargs rm")); // consumer that spawns commands
        assert!(!allowed("orx runs p-1 | sed -i s/x/y/ f")); // sed excluded (writes)
        assert!(!allowed("orx runs p-1 | awk '{system(\"id\")}'")); // awk excluded
        assert!(!allowed("orx runs p-1 | head > /tmp/x")); // redirect in a stage
        assert!(!allowed("orx runs p-1 || rm -rf /")); // `||` is not a pipe
        assert!(!allowed("orx runs p-1 |")); // trailing pipe
        assert!(!allowed("| head")); // no producer
        assert!(!allowed("cargo metadata | head")); // unknown producer
        assert!(!allowed("head -1 f | orx runs p-1")); // orx is not a consumer
    }

    #[test]
    fn git_commands_require_approval_even_for_ordinary_reads() {
        for c in [
            "git status",
            "git log --oneline -20",
            "git show HEAD~1:src/main.rs",
            "git diff --stat main..feature",
            "git grep -n TODO",
            "git ls-tree -r --name-only origin/main",
            "git ls-files",
            "git rev-parse HEAD",
            "git rev-list --count HEAD",
            "git cat-file -p HEAD:README.md",
            "git blame src/main.rs",
            "git describe --tags",
            "git merge-base main feature",
            "git -P log -5",
            "git --no-pager diff",
            "git -C /some/repo log --oneline",
            "git branch -a",
            "git branch --list 'daniel/*'",
            "git branch --show-current",
            "git tag",
            "git tag -l 'v*'",
            "git stash list",
            "git stash show",
            "git remote -v",
            "git remote show origin",
            "git remote get-url origin",
            "git worktree list",
            "git reflog",
            "/usr/bin/git status",
        ] {
            assert!(!allowed(c), "should gate: {c}");
        }
    }

    #[test]
    fn git_writes_are_gated() {
        for c in [
            "git push",
            "git commit -m x",
            "git checkout main",
            "git switch -c feat",
            "git add .",
            "git reset --hard HEAD~1",
            "git fetch origin",
            "git pull",
            "git merge feature",
            "git rebase main",
            "git branch foo",                           // creates
            "git branch -d foo",                        // deletes
            "git branch -m old new",                    // renames
            "git branch --set-upstream-to=origin/main", // config write
            "git tag v1.0",                             // creates
            "git tag -d v1.0",                          // deletes
            "git tag -a v1 -m msg",                     // creates annotated
            "git stash",                                // bare stash = push
            "git stash pop",
            "git remote add origin url",
            "git remote set-url origin url",
            "git worktree add /tmp/wt",
            "git worktree remove /tmp/wt",
            "git reflog expire --all",
            "git config user.name x", // excluded wholesale
            "git",                    // bare usage: no reason to allow
            // Escape hatches on otherwise-read verbs.
            "git -c core.pager='!id' log", // -c injects executable config
            "git --git-dir=/x/.git log",   // repo redirection
            "git log --output=/tmp/exfil", // writes a file
            "git grep -Oless TODO",        // executes a pager
            "git grep --open-files-in-pager TODO",
        ] {
            assert!(!allowed(c), "should gate: {c}");
        }
    }

    #[test]
    fn non_bash_tools_defer() {
        let p = json!({ "tool_name": "Edit", "tool_input": { "command": "orx runs p-1" } });
        assert!(decide(&p).is_none());
        // Missing command field → defer, not panic.
        let p = json!({ "tool_name": "Bash", "tool_input": {} });
        assert!(decide(&p).is_none());
    }

    #[test]
    fn exit_plan_mode_asks_instead_of_self_approving() {
        let p = json!({ "tool_name": "ExitPlanMode", "tool_input": { "plan": "do X" } });
        let out = decide(&p).expect("ExitPlanMode must get a decision");
        assert_eq!(
            out.pointer("/hookSpecificOutput/permissionDecision")
                .and_then(Value::as_str),
            Some("ask")
        );
    }

    #[test]
    fn allow_decision_has_the_exact_wire_shape() {
        let out = decide(&payload("orx runs p-1")).unwrap();
        assert_eq!(
            out.pointer("/hookSpecificOutput/hookEventName")
                .and_then(Value::as_str),
            Some("PreToolUse")
        );
        assert_eq!(
            out.pointer("/hookSpecificOutput/permissionDecision")
                .and_then(Value::as_str),
            Some("allow")
        );
    }

    /// Drift guard: every whole-verb read on the allowlist must still name a
    /// real top-level `orx` command. A rename in `main.rs` breaks this test
    /// (instead of silently un-gating nothing). Catching *additions* of new read
    /// verbs is the human's job — see the pointer comment on the `Command` enum.
    #[test]
    fn readonly_verbs_are_real_commands() {
        use clap::CommandFactory;
        let cmd = crate::Cli::command();
        let real: std::collections::HashSet<&str> =
            cmd.get_subcommands().map(|s| s.get_name()).collect();
        for verb in WHOLE_VERB_READS {
            assert!(
                real.contains(verb),
                "allowlisted read verb `{verb}` is not a real orx command \
                 (renamed in main.rs?)"
            );
        }
        // The verbs with mixed read/write subcommands must also be real.
        for verb in ["project", "exp"] {
            assert!(real.contains(verb), "`{verb}` is not a real orx command");
        }
    }
}
