# Changelog

All notable changes to zeo-systray are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

A version is the `version` in `Cargo.toml`, and a release is the git tag
`v<X.Y.Z>` on the commit that sets it. Changes are recorded under
`## [Unreleased]` as they land, and that section is renamed to
`## [X.Y.Z] — YYYY-MM-DD` when the tag is cut.

The tray is fed by Claude Code hooks and, optionally, by Zeo itself over
`$XDG_RUNTIME_DIR/zeo-systray.sock`. Zeo's side of that channel is patch 0038
in `zed-patches`, and it needs this daemon at 0.3.0 or later.

## [Unreleased]

## [0.3.0] — 2026-10-04

Zeo can now tell the tray when one of its agent's background tasks ends out of
view, so a shell command or subagent that finishes or fails while you are
looking elsewhere raises a notification (story 013).

### Added

- When Zeo reports that a background task ended — one you asked it to watch, or any task that failed — the tray shows a notification titled `Task <outcome> — <project>` (completed, failed, stopped or interrupted), with the thread's title and how long the task ran. Clicking it opens that thread, the same way a menu row does. The event also lands in "Recent notifications", and "Silence notifications" stops the popup but not the record. Nothing needs to be configured on the tray's side; Zeo sends it directly (patch 0038, Zeo 0.1.1 and later).

### Changed

- The repository gained a security policy (`SECURITY.md`) and Dependabot updates for its Cargo dependencies; the daemon itself is unaffected.

## [0.2.2] — 2026-10-01

### Changed

- Clicking a session or a notification hands the thread link to the editor that is installed — `zeo` (from `app-editors/zeo` or `zeo-bin`), then `zedit` (`zed`), then `zedit-bin` (`zed-bin`) — and falls back to `xdg-open` only when none is on `PATH`. With Zeo installed in place of Zed, clicks used to open nothing. The KDE opener script does the same.
- When the program that opens a thread fails, its exit status is now logged; before, a failure left no trace.

### Fixed

- The "Open thread" button stays on a notification. In 0.2.1, Plasma removed it from every popup the tray raised.
- The daemon builds again with the minimum Rust version `Cargo.toml` declares (1.85). Packaged builds were not affected.

## [0.2.1] — 2026-10-01

### Fixed

- A permission prompt raises one notification instead of two.
- Clicking a notification opens only the thread it names. After Plasma's notification service restarted, a single click could open two or three threads at once — the one clicked plus older threads whose notifications had expired into the history. The daemon also no longer keeps a waiting thread alive for every notification it ever showed.

## [0.2.0] — 2026-09-29

The tray now watches the Claude plan's usage limits and tells you as they fill,
without needing a credential or network access.

### Added

- A notification every 5% of the five-hour usage limit and every 10% of the weekly limit and of each per-model weekly limit. The current figures are shown in the tooltip and at the top of the menu. A step is announced once, even when two sources disagree by a point or the figure wobbles; the first reading after the daemon starts is taken silently, and a limit that resets counts from zero again.
- The figures come from two places: the new `zeo-systray statusline` command, set as Claude Code's `statusLine`, which covers terminal sessions and the TUI adapter (five-hour and weekly only); and the usage sample `claude-agent-acp-plus` writes under `$XDG_RUNTIME_DIR`, the only source of the per-model limits.

## [0.1.6] — 2026-09-29

### Fixed

- The KDE opener finds the project's window for a session running in a subdirectory of the project, by trying the directory's name and then each parent's, nearest first, stopping before `$HOME`. Before, such a session found no window and its thread opened on a desktop nobody was looking at.
- The KDE opener passes the link to `zedit` when it is installed instead of `xdg-open`, which could pick another editor for `zed://`; `ZEO_SYSTRAY_KDE_LINK_OPENER` overrides it. It also raises the window a second time a second later, to cover a project the editor had to open a new window for.

## [0.1.5] — 2026-09-29

### Fixed

- A thread opened from the tray carries its session's directory (`&cwd=…`), so the editor can open it in the window holding that project rather than whichever window had focus, where it failed with "no thread found". This pairs with patch 0024; an editor without it ignores the extra parameter. Custom open commands gain a `{cwd_query}` placeholder, the directory encoded for a URL query; `{cwd}` stays the raw path.

## [0.1.4] — 2026-09-14

### Fixed

- Under systemd, clicking a session no longer shows a KDE error about `~/.config/kde-openrc` not being writable. The program that opens the thread now runs outside the daemon's sandbox, so it — and the editor it may start — get normal file and network access. Nothing changes under OpenRC.

## [0.1.3] — 2026-09-14

### Fixed

- The shipped systemd user unit starts. Its hardening made the runtime directory read-only, so the daemon could not create its socket and restarted endlessly — over 4000 times in a day on 0.1.2.

## [0.1.2] — 2026-09-14

### Added

- A KDE opener script, `contrib/zeo-systray-open-kde.sh`: clicking a session switches to the virtual desktop holding the project's window, raises it, and then opens the thread, so you see the switch happen. It matches the window by the project's directory name and is wired in through `ZEO_SYSTRAY_OPEN_CMD`. KDE Plasma only.

## [0.1.1] — 2026-09-13

### Added

- Session rows in the menu open the thread they name (`zed://agent?session=<id>`). `ZEO_SYSTRAY_OPEN_CMD` replaces the command, with `{session}` and `{cwd}` substituted; it is run directly, never through a shell.
- A "Recent notifications" submenu with the last twenty events, newest first, labelled with how long ago they happened.
- "Silence notifications" stops the popups and keeps recording, so the history can be read later.
- Every notification has an "Open thread" action, and middle-clicking the icon clears finished sessions.

### Changed

- Permission requests are raised as critical notifications, so the desktop does not dismiss them on a timer while the agent waits.

### Fixed

- The icon shows right after installing the package, instead of only after the next login.

## [0.1.0] — 2026-09-13

The first release: a tray icon that tells you when a Claude Code agent session
finishes a turn, needs a permission decision, or is waiting on background work,
when its window is not in front of you.

### Added

- One binary with two modes. `zeo-systray notify`, run from Claude Code hooks, sends one small message to the daemon and exits; it always succeeds, so a tray that is not running never shows up as an error inside an agent session. `zeo-systray daemon` shows a StatusNotifierItem tray icon with a status badge, a menu of sessions, and desktop notifications. It speaks to the desktop over D-Bus directly, with no GTK or libappindicator, and keeps its state in memory only.
- Only identifiers, a project label, counters and the short text Claude writes for display reach the daemon. The assistant's messages, tool inputs, shell command lines and the transcript path stay in the hook.
