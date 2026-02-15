# CI/CD Configuration

## CircleCI

**Project:** gh/graybear-io/gbe-transport
**Dashboard:** https://app.circleci.com/pipelines/gh/graybear-io/gbe-transport

### For Agents

When using CircleCI MCP tools, use these parameters:

```json
{
  "projectSlug": "gh/graybear-io/gbe-transport",
  "gitRemoteURL": "git@github.com:graybear-io/gbe-transport.git",
  "workspaceRoot": "/Users/bear/projects/gbe-transport",
  "branch": "main"
}
```

### Common Tasks

**Check build status:**
- Tool: `get_latest_pipeline_status`
- Parameters: `projectSlug` + `branch`

**Get build failures:**
- Tool: `get_build_failure_logs`
- Parameters: `projectSlug` + `branch`

**Get test results:**
- Tool: `get_job_test_results`
- Parameters: `projectSlug` + `branch`

**Trigger pipeline:**
- Tool: `run_pipeline`
- Parameters: `projectSlug` + `branch`

### Workflows

**1. Check CI Status**
```bash
# Agent uses: get_latest_pipeline_status
# Human uses: gh pr checks (if PR) or visit dashboard
```

**2. Debug Test Failures**
```bash
# Agent uses: get_build_failure_logs + get_job_test_results
# Human uses: Click into failed job on dashboard
```

**3. Trigger Manual Build**
```bash
# Agent uses: run_pipeline
# Human uses: "Rerun workflow" button on dashboard
```

## Configuration Files

**CircleCI config:** `.circleci/config.yml`

### Quality Gates

Phase 1 requirements:
- ✅ All tests passing (`cargo test --workspace`)
- ✅ Clippy clean (`cargo clippy --workspace`)
- ✅ Formatting clean (`cargo fmt --check`)
