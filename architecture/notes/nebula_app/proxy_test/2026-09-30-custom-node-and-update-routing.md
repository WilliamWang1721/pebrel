# Custom test nodes and update routing

## Status

Proposed.

## Context

The network settings page tested only `example.com:80`. Update requests always
resolved system/environment proxies independently of the network page.
Users need to select their test destination and turn update proxy use on or off.

## Evidence

`ssh_session::proxy_test_once` already owns the bounded asynchronous test and SSH
proxy handshakes. `update_proxy::agent` owns clients for release checks, the API
rate-limit fallback, checksum metadata and package downloads. Settings and reset
contracts belong to `nebula_settings`.

## Decision

Persist `network_test_url` and `update_proxy` in the existing settings file. The
defaults remain `http://example.com/` and enabled update proxy use, preserving
old files. The shared settings reader owns defaults; reset removes both keys.

Accept HTTP/HTTPS URLs with a port, path and query; reject credentials and fragments.
Use the existing proxy byte stream for the selected host and port. For HTTPS,
wrap it with certificate-verified TLS using the already locked tokio-rustls and
WebPKI roots. The application gains direct dependency edges to these two existing
packages; the settings crate retains zero production dependencies.

The network pane saves on Enter/blur and before testing. Actual text edits
invalidate the existing request generation; unchanged Change notifications after
Enter retain validation feedback. Old results cannot describe a changed node. The
existing twelve-second total deadline also bounds DNS, proxy and TLS work.

Update proxy use is independent of terminal proxy use. Disabled means an explicit
`None` proxy, overriding environment detection. Enabled custom mode uses the
network proxy URL and exclusions, reusing the existing parser for legacy bare
SOCKS5 addresses, protocol default ports and encoded credentials; other modes
retain existing automatic resolution. IPv6 hosts retain URL brackets; credentials
that the update client cannot preserve return an error rather than being changed.
Invalid custom proxies are errors, not a silent direct fallback. Each update
operation reads current settings when its client is created; running downloads
keep their client. No process environment is mutated.

## Rejected alternatives

- A second tester/updater service would duplicate transport and task ownership.
- Requiring HTTPS tests to bypass the selected route would not test that proxy.
- Changing legacy defaults would silently alter existing update traffic.

## Consequences

The product GPUI network page exposes both preferences. HTTP 200–499 continues
to mean reachable; the test does not assert application health or follow redirects.
Other languages fall back to English for new labels until translated.

## Validation

Settings round-trip/reset tests, URI validation, actual local direct/HTTP CONNECT/
SOCKS5 destination tests, updater selection tests and a rendered keyboard/switch
fixture cover the changed contracts. Platform build and visual coverage are
reported separately in the PR.

## Supersedes

None.

## Revisit when

The updater must support SSH jumps/custom commands, or test results require a
different success criterion. Such transports are not updater HTTP proxies.
