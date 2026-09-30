# Notification channel rules

## Status

Proposed with the notification tools settings change.

## Context

Users need native notifications while viewing a terminal and want to select one
channel or different foreground/background channels by event type.

## Evidence

The GPUI workspace previously decided both channels from source visibility, while
application messages and update cards always used in-app delivery. The AI visibility
preference only controls in-app AI cards and must remain compatible.

## Decision

`nebula_settings::NotificationRouting` owns channel defaults, rule keys, parsing
and routing. The existing key/value format adds a mode and ten optional rule keys;
missing or invalid values preserve automatic routing. UI adapters map typed task
outcomes to categories and use cached routing without disk reads per notification.
Application messages use window focus; pane events retain their verified visibility.
Explicit system and mixed modes dispatch while foregrounded. Existing native workers,
action callbacks, throttling and confirmation validation remain authoritative.

## Rejected alternatives

- A second routing implementation per notification adapter: would drift on defaults.
- Arbitrary expressions or keywords: the requested scope is event type and visibility.
- Disabling approvals when notifications are off: delivery must not mutate tasks.
- Changing the default to native-only: would silently change existing preferences.

## Consequences

Rules apply to newly delivered notifications; retained cards keep their lifetime.
The legacy AI switch can suppress the in-app part of mixed AI delivery. Terminal
BEL effects remain separately controlled by the bell setting. Native permissions
and system suppression still apply. Save-error feedback remains in Settings so a
muted or unavailable channel cannot hide a rejected preference write. Update native
notifications focus the app; update actions remain available in Application settings.

## Validation

Routing tests cover explicit channels, custom rules, defaults, invalid values,
persistence and reset. Workspace tests cover foreground delivery and the legacy AI
switch; GPUI tests cover searchable controls, menu keyboard access, live settings
and rejected writes. Platform delivery still requires native OS acceptance.

## Supersedes

None; extends the existing notification lifetime contract.

## Revisit when

More event types or verified native capabilities require a rule beyond type and
visibility, or the legacy AI visibility preference is migrated separately.
