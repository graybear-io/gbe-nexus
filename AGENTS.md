# Agent Instructions

## Task Tracking

We use **Taskwarrior** (`task`) for all task and issue tracking, driven by the `/tw` skill.

### Work Loop

1. `task ready` — find unblocked tasks (sorted by urgency)
2. `task <id> info` — read full description and annotations
3. `task <id> start` — mark active
4. Do the work
5. `task <id> done` — mark complete
6. `task ready` — repeat until empty

## Before Every Commit (MANDATORY)

**Quality gates MUST run before every commit, not just at session end.**

**NO EXCEPTIONS** - Even for "small" changes or documentation.

## Commit Workflow

Install, if needed, into the repository a pre-commit hook that **automatically** runs quality gates:

```bash
git add <files>
git commit -m "message"
# → Hook runs: cargo fmt --check, clippy, test
# → Commit proceeds only if all pass
```

**To bypass** (emergency only):

```bash
git commit --no-verify -m "message"
```

## Manual: Quality Gates Checklist

If you need to run checks manually:

```bash
just ci

# Commit (hook will verify)
git add <files>
git commit -m "message"
```

## Troubleshooting

Formatting check failed:

```bash
cargo fmt
git add <files>
git commit -m "message"
```

Clippy check failed:

```bash
cargo clippy --workspace --fix --allow-dirty
git add <files>
git commit -m "message"
```

Tests failed:

- Fix the failing tests first
- Or commit with `--no-verify` if intentionally committing broken tests (rare)

## Hook Installation

Pre-commit hook is at `.git/hooks/pre-commit`.

**To disable:**

```bash
rm .git/hooks/pre-commit
```

**To enable:**

```bash
chmod +x .git/hooks/pre-commit
```

**Not in git?** Hooks are local-only (`.git/hooks/` not tracked). Document setup in README or project setup guide.

## Quick Reference

```bash
task ready                    # Find unblocked, actionable tasks
task <id> info                # Full detail + annotations
task <id> start               # Mark as active (in progress)
task <id> done                # Mark complete
task <id> annotate "context"  # Attach notes
task add "desc" project:gbe-nexus priority:H  # Create task
task add "desc" depends:<id>  # Create with dependency
```

### Closing Work

```bash
task <id> done                              # Mark complete
task <id> annotate "Findings: details here" # Add context
```

### Core Rules

- Use Taskwarrior for ALL task, bug and issue tracking
- Use `depends:<id>` to link discovered/related work
- Check `task ready` before asking "what should I work on?"
- Use `task <id> annotate` to attach context, not separate docs
- Run `task <cmd> --help` to discover available flags
- do NOT create markdown TODO lists
- do NOT use external issue trackers
- do NOT duplicate tracking systems

### Error Handling

CRITICAL: Always communicate errors and fixes
NEVER: Suppress or silently handle errors -- Every error must be communicated to the user.

When a command fails:

1. **Output what went wrong** - Show the error to the user
1. **Explain what you discovered** - If you found the correct approach, explain it
1. **Try the fix** - If you know the correct parameter/command, use it
1. **If uncertain, STOP and ASK** - Don't guess

**Before running task commands:**

- If uncertain about flags, run `task <command> --help` FIRST
- Use `task <id> done` to complete tasks, `task <id> annotate` for notes

Examples:

✅ **Good - Discovered correct parameter:**
> "The `task modify` command failed with an unknown flag. Let me run `task modify --help` to find the right syntax."

✅ **Good - Uncertain about fix:**
> "I got an error setting dependencies. Should I use `depends:3` or a different syntax?"

❌ **Bad - Silent workaround:**
> [Command fails, tries different approach without mentioning the error]

❌ **Bad - Guessing at flags:**
> [Tries unknown flags without checking `--help` first]

When in doubt, STOP and ASK.

### Sequential Command Validation

CRITICAL: Check each step before proceeding to the next.

When running dependent commands (where B depends on A succeeding):

**REQUIRED WORKFLOW:**

1. Run command A
1. **CHECK the result** - Look for errors, exit codes, or failure messages
1. **If A failed:**
   - STOP immediately
   - Communicate the error to the user
   - Fix the issue before continuing
1. **If A succeeded:**
   - Proceed to command B

**Use `&&` for dependent shell commands:**

```bash
# GOOD - Chain stops on first failure
git add file.md && git commit -m "message" && git push

# BAD - Each runs regardless of previous failures
git add file.md
git commit -m "message"  # Commits nothing if add failed
git push                  # Pushes nothing if commit failed
```

**Examples:**

✅ **Good - Validates each step:**

```text
I'll add the file to git:
[runs: git add history/doc.md]
[checks result: sees "fatal: pathspec not found"]
"git add failed - it can't find the file at that path. I'm in mdg/
so git is looking for mdg/history/ not services/history/. Let me cd
to the repo root first."
[fixes issue and retries]
```

❌ **Bad - Ignores failure and continues:**

```text
[runs: git add history/doc.md]
[sees error but ignores it]
[runs: git commit]
[runs: git push]
Result: Nothing was committed or pushed, error hidden from user
```

**Tool Result Validation:**

After EVERY tool call, check the result:

- **Bash**: Check for `error` fields, exit codes, error messages in output
- **Edit/Write**: Check for success confirmation
- **Read**: Check for file not found errors
- **Grep/Glob**: Check for "No files found" or empty results when you expected matches

**If you see an error in a tool result:**

1. STOP immediately (don't run the next command)
1. Tell the user what failed
1. Explain what you think went wrong
1. Fix it or ask for help

When in doubt, STOP and ASK

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File remaining work** - `task add` for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update task status** - `task <id> done` for finished work, annotate in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:

   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```

5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**

- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
