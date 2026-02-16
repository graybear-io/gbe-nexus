# Agent Instructions

## Issue Tracking

We use **bd (beads)** for issue and task tracking.

**Get full workflow context:** Run `bd prime` for dynamic, up-to-date workflow instructions (~80 lines).
Hooks auto-inject this at session start when `.beads/` is detected.

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
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

### Updating Issues

**When closing work (Two-Step Process):**

**Step 1 - Close with brief reason:**

```bash
bd close <id> --reason "Brief topic/summary"
```

**Step 2 - Add detailed notes:**

```bash
bd update <id> --notes "Detailed findings, context, links to artifacts"
```

**IMPORTANT**: These are **separate commands**. `bd close` does NOT accept `--notes` flag.

**Field usage:**

- `--reason`: Brief topic/summary (only with `bd close`)
- `--notes`: Detailed findings, multi-line context, links (only with `bd update`)
- **NEVER use `--status completed`** - use `bd close` instead
- Valid statuses: open, in_progress, closed, blocked

### Core Rules

- Use bd for ALL task, bug and issue tracking
- Always use --json flag for programmatic use
- link discovered work with `discovered-from` dependencies
- check `bd ready` before asking "what should I work on?"
- store AI planning docs in `history/` directory
- run `bd <cmd> --help` to discover available flags
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

**Before running bd commands:**

- If uncertain about flags, run `bd <command> --help` FIRST
- Common mistake: `bd close` does NOT accept `--notes` (see "Updating Issues" section)
- Common mistake: Combining flags from different commands (use separate commands)

Examples:

✅ **Good - Discovered correct parameter:**
> "The `bd update` command failed with '--comment: unknown flag'. Looking at the available flags, I see `--notes` is the correct parameter for detailed context. Let me try that instead."
> [proceeds to use --notes]

✅ **Good - Uncertain about fix:**
> "I got error 'invalid status: completed'. I'm not sure what the valid statuses are. Should I use `bd close` instead, or is there a different status value I should use?"

❌ **Bad - Silent workaround:**
> [Command fails, tries different approach without mentioning the error]

❌ **Bad - Guessing at flags:**
> [Tries `bd close --notes` without checking if the flag exists]

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

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:

   ```bash
   git pull --rebase
   bd sync
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
