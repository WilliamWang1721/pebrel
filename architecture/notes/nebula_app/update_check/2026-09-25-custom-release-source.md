# Configurable GitHub Release update source

## Status

Proposed for review; implemented on top of the existing updater.

## Context

Pebrel's updater previously bound discovery and download validation to the
official `Kuddev/pebrel` stable release. Fork users and prerelease testers could
not keep the existing in-app check, verified download, and installation handoff
while selecting their own GitHub Release.

## Decision

Add one persisted `update_release_url` preference. An empty value preserves the
existing official stable source. A custom value must be an HTTPS
`github.com/<owner>/<repo>` release location and is normalized to either that
repository's Releases page or an explicit `releases/tag/<tag>` page.

Release discovery still uses the existing GitHub API/proxy path. Repository
release pages select GitHub's latest stable release; an explicit tag selects that
exact release, including semver-style beta/preview tags. API rate-limit fallback
stays inside the same selected repository and uses its `SHA256SUMS` asset when
available.

The custom setting changes only the repository trust boundary. Existing native
asset-name selection, package size limits, SHA-256 requirements, streamed
verification, platform installation checks, and handoff remain unchanged. A
cached package is rejected if it no longer belongs to the currently configured
source, so cache metadata cannot authorize a different repository by itself.

## Compatibility

Existing settings omit `update_release_url` and therefore keep the official
behavior. Resetting settings removes the override. The existing version policy
still prevents downgrades and does not reinterpret same-version prereleases.
