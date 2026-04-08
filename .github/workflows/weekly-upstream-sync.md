---
on:
  schedule: weekly
  workflow_dispatch:

description: "Weekly sync from nix-community/lanzaboote:master without merge commits"

permissions:
  contents: read
  pull-requests: read

engine:
  id: claude
  env:
    ANTHROPIC_BASE_URL: "https://api.z.ai/api/anthropic"

network:
  allowed:
    - api.z.ai
    - defaults
    - github

concurrency:
  group: gh-aw-weekly-upstream-sync-${{ github.ref_name }}
  cancel-in-progress: false

safe-outputs:
  jobs:
    upstream-sync:
      description: "Sync master with nix-community/lanzaboote:master without merge commits"
      runs-on: ubuntu-latest
      permissions:
        contents: write
        pull-requests: write
      output: "Upstream sync job completed."
      inputs:
        base_branch:
          description: "Branch to keep in sync"
          required: true
          type: string
        upstream_repo:
          description: "Upstream repository in owner/name format"
          required: true
          type: string
        upstream_branch:
          description: "Upstream branch to sync from"
          required: true
          type: string
        sync_branch:
          description: "Branch name to reuse for conflict PRs"
          required: true
          type: string
        pr_title:
          description: "Title to use when opening a conflict PR"
          required: true
          type: string
        pr_body:
          description: "Body to use when opening a conflict PR"
          required: true
          type: string
      steps:
        - name: Check out repository
          uses: actions/checkout@v4
          with:
            fetch-depth: 0
            persist-credentials: true
            token: ${{ secrets.GH_AW_SYNC_TOKEN }}

        - name: Sync with upstream
          env:
            GH_TOKEN: ${{ secrets.GH_AW_SYNC_TOKEN }}
          run: |
            set -euo pipefail

            payload="$(cat "$GH_AW_AGENT_OUTPUT")"

            base_branch="$(jq -r '.items[] | select(.type == "upstream_sync") | .base_branch' <<<"$payload")"
            upstream_repo="$(jq -r '.items[] | select(.type == "upstream_sync") | .upstream_repo' <<<"$payload")"
            upstream_branch="$(jq -r '.items[] | select(.type == "upstream_sync") | .upstream_branch' <<<"$payload")"
            sync_branch="$(jq -r '.items[] | select(.type == "upstream_sync") | .sync_branch' <<<"$payload")"
            pr_title="$(jq -r '.items[] | select(.type == "upstream_sync") | .pr_title' <<<"$payload")"
            pr_body="$(jq -r '.items[] | select(.type == "upstream_sync") | .pr_body' <<<"$payload")"

            if [ -z "$base_branch" ] || [ -z "$upstream_repo" ] || [ -z "$upstream_branch" ] || [ -z "$sync_branch" ]; then
              echo "Missing required sync parameters"
              exit 1
            fi

            git config user.name "github-actions[bot]"
            git config user.email "41898282+github-actions[bot]@users.noreply.github.com"

            git remote remove upstream 2>/dev/null || true
            git remote add upstream "https://github.com/${upstream_repo}.git"

            git fetch --no-tags origin "${base_branch}"
            git fetch --no-tags upstream "${upstream_branch}"

            origin_ref="refs/remotes/origin/${base_branch}"
            upstream_ref="refs/remotes/upstream/${upstream_branch}"
            origin_sha="$(git rev-parse "${origin_ref}")"
            upstream_sha="$(git rev-parse "${upstream_ref}")"

            echo "origin ${base_branch}: ${origin_sha}"
            echo "upstream ${upstream_branch}: ${upstream_sha}"

            if [ "${origin_sha}" = "${upstream_sha}" ]; then
              echo "Branch is already up to date."
              exit 0
            fi

            work_branch="gh-aw-sync-work"
            git checkout -B "${work_branch}" "${origin_ref}"

            if git merge-base --is-ancestor "${origin_sha}" "${upstream_sha}"; then
              echo "Fast-forwarding ${base_branch} to ${upstream_sha}"
              git push origin "${upstream_sha}:refs/heads/${base_branch}"
              exit 0
            fi

            set +e
            git rebase "${upstream_ref}"
            rebase_status=$?
            set -e

            if [ "${rebase_status}" -eq 0 ]; then
              rebased_sha="$(git rev-parse HEAD)"
              echo "Rebased ${base_branch} to ${rebased_sha}"
              git push --force-with-lease="refs/heads/${base_branch}:${origin_sha}" origin "${rebased_sha}:refs/heads/${base_branch}"
              exit 0
            fi

            echo "Rebase hit conflicts. Opening or updating a PR instead."
            git rebase --abort || true

            git checkout -B "${sync_branch}" "${upstream_ref}"
            git push --force origin "${sync_branch}:refs/heads/${sync_branch}"

            existing_pr="$(gh pr list \
              --base "${base_branch}" \
              --head "${sync_branch}" \
              --state open \
              --json number \
              --jq '.[0].number // empty')"

            if [ -n "${existing_pr}" ]; then
              gh pr edit "${existing_pr}" --title "${pr_title}" --body "${pr_body}"
              echo "Updated existing PR #${existing_pr}"
            else
              repo_id="$(gh repo view "${GITHUB_REPOSITORY}" --json id --jq .id)"
              gh api graphql \
                -f query='mutation($repo:ID!,$base:String!,$head:String!,$title:String!,$body:String!){createPullRequest(input:{repositoryId:$repo,baseRefName:$base,headRefName:$head,title:$title,body:$body}){pullRequest{url number}}}' \
                -F repo="${repo_id}" \
                -F base="${base_branch}" \
                -F head="${sync_branch}" \
                -F title="${pr_title}" \
                -F body="${pr_body}"
            fi
---

# Weekly Upstream Sync

Keep this repository's `master` branch aligned with `nix-community/lanzaboote:master` on a weekly cadence.

## Goal

Update the default branch without merge commits.

- If this repository is simply behind upstream, fast-forward `master`.
- If this repository has local commits that can be replayed cleanly, rebase them onto upstream and update `master` with a linear history.
- If the rebase would conflict, do not resolve conflicts automatically. Open or update a pull request from a dedicated sync branch so a maintainer can resolve it manually.

## Token

Use the `GH_AW_SYNC_TOKEN` repository secret for pushing branches and creating pull requests.

## Instructions

1. Fetch `origin/master` and `nix-community/lanzaboote:master`, then inspect the relationship between the two histories with `git merge-base`, `git log`, and similar read-only commands.
2. Never create merge commits. Do not use `git merge` for the sync.
3. If the branch is already current, explain that no action is required and stop.
4. Otherwise call the `upstream-sync` tool once with:
   - `base_branch`: `master`
   - `upstream_repo`: `nix-community/lanzaboote`
   - `upstream_branch`: `master`
   - `sync_branch`: `gh-aw/upstream-sync-master`
   - `pr_title`: `Sync master with nix-community/lanzaboote:master`
   - `pr_body`: a short explanation that the automated weekly sync hit rebase conflicts, no merge commit was created, and the PR exists for manual conflict resolution.
5. Keep the final response concise and factual. State whether the repository was already up to date, was synced directly, or required a conflict PR.
