# YubiKey PIV authentication through the SSH agent

## Status

Proposed.

## Context

Pebrel's built-in russh authentication did not consult `SSH_AUTH_SOCK`. A PIV key loaded by OpenSSH through Yubico's PKCS#11 provider therefore remained unavailable to in-app SSH sessions.

## Evidence

The existing `Auto` plan tried configured key files, stored password, keyboard-interactive and prompted password only. russh already exposes an SSH-agent signer, while YKCS11 can be loaded into the current OpenSSH agent with `ssh-add -s`.

## Decision

On Unix, `Auto` tries identities from the current SSH agent after explicit key files and before password methods. Pebrel delegates signing to russh's agent client; it does not read hardware private keys.

A small `pebrel yubikey [--provider PATH]` command on macOS and Linux runs `ssh-add -s` for YKCS11. Common provider locations are detected when no path is supplied.

No new authentication mode, dependency, settings state, PIN storage, key generation, key import or persistent helper process is added.

## Rejected alternatives

- Direct PKCS#11 signing inside Pebrel: duplicates agent/provider responsibilities and requires additional integration surface.
- A Pebrel-managed persistent agent: adds lifecycle and state ownership that the feature does not need.
- A dedicated YubiKey settings/UI mode: unnecessary for the first usable authentication path.

## Consequences

Hardware signing stays owned by the user's SSH agent and YubiKey provider. If no Unix agent is available, `Auto` continues with the existing password methods. The YKCS11 loader is not exposed on Windows.

## Validation

The existing authentication-plan test covers the new ordering. Architecture contracts and the repository's native cross-platform test matrix validate the software path. GitHub-hosted CI has no physical YubiKey, so PIN/touch hardware acceptance is not claimed.

## Supersedes

None.

## Revisit when

Revisit if Pebrel adopts a direct hardware-key signer, needs a Windows YKCS11 loader, or changes the ownership of SSH-agent authentication.
