#!/usr/bin/env bash
# Open an agent thread AND bring its editor window to the front, on KDE.
#
# Point the tray at this script:
#
#     ZEO_SYSTRAY_OPEN_CMD='/usr/share/zeo-systray/zeo-systray-open-kde.sh {session} {cwd}'
#
# Why a script instead of doing it in the daemon: raising a window is not
# something an application can decide on Wayland. The compositor rejects an
# activation request from a process that does not already have focus -- that is
# the whole point of focus-stealing prevention -- so the editor asking to be
# raised gets, at best, a blinking task-bar entry. What *can* do it is the
# compositor itself. This runs a throwaway KWin script, which executes inside
# KWin and is therefore not a foreign process asking for focus.
#
# That makes this file KDE-specific by nature, which is also why it is not
# compiled into the daemon: a tray that works on six desktops should not carry
# one desktop's window manager inside it.
#
# Exit is always 0: an opener that fails must never look like an agent failure.

set -uo pipefail

session="${1:-}"
cwd="${2:-}"

[[ -n "${session}" ]] || exit 0

# Which window holds the thread is the editor's to say, not ours to guess. A
# window can hold several projects and its caption names only the one in front,
# and two windows can share a caption, so matching captions raised whichever
# window came first. Handed the link, the editor asks the compositor to activate
# the window that holds the thread. KWin grants that when the editor already
# has focus, and otherwise refuses it and marks that same window as demanding
# attention. Either way the window shows itself, so the KWin script waits for it
# and brings the one the editor named.
#
# The caption is only the fallback, for an editor that asks for nothing. It is
# "<project> — <open file>", and the file half changes whenever a tab does, so
# only the project half is compared. The hook reports the directory the session
# runs in, which is the project root or somewhere below it -- a session started
# in app-emulation/d7vk of a project named bentoo -- so the candidates are that
# directory's name and then each parent's, nearest first, stopping short of
# $HOME. The nearest candidate any window matches decides, and it must match
# exactly one window: with two, a guess is as likely to be wrong as right.
candidates=()
dir="${cwd%/}"
while [[ -n "${dir}" && "${dir}" != "/" && "${dir}" != "${HOME%/}" ]]; do
	candidates+=("$(basename -- "${dir}")")
	dir="$(dirname -- "${dir}")"
done

# How long the editor gets to name its window before the caption is tried, and
# how long the script stays loaded after the link -- long enough for a window
# the editor had to open for a project no window held.
fallback_ms=3000
watch_seconds=5

watcher_name="zeoSystrayRaise$$"
watcher_js=""

start_watcher() {
	command -v busctl >/dev/null 2>&1 || return 0

	# Embedded as JSON so a directory name with quotes cannot break out of the
	# script it is pasted into.
	local json_candidates
	json_candidates="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "${candidates[@]}" 2>/dev/null)" || return 0

	watcher_js="$(mktemp --suffix=.js)" || return 0

	cat >"${watcher_js}" <<-EOF
		var wanted = ${json_candidates};
		var done = false;

		function isEditor(w) {
		    var cls = String(w.resourceClass || "").toLowerCase();
		    return cls.indexOf("zed") !== -1 || cls.indexOf("zeo") !== -1;
		}

		function bring(w) {
		    if (done) return;
		    done = true;
		    // Switching desktop first is the part that matters: activating a
		    // window the screen is not showing leaves the user looking at the
		    // desktop they were already on.
		    //
		    // With "switch desktops independently for each screen" (KWin >= 6.8,
		    // [Windows] PerOutputVirtualDesktops), assigning currentDesktop moves
		    // every screen at once: the other monitor loses what it was showing.
		    // Switching only the screen that holds the window keeps it. The
		    // argument order is (desktop, output) -- the reverse is a type error.
		    if (w.desktops && w.desktops.length > 0) {
		        if (typeof workspace.setCurrentDesktopForScreen === "function" && w.output) {
		            workspace.setCurrentDesktopForScreen(w.desktops[0], w.output);
		        } else {
		            workspace.currentDesktop = w.desktops[0];
		        }
		    }
		    if (w.minimized) {
		        w.minimized = false;
		    }
		    workspace.activeWindow = w;
		}

		// Granted: KWin activated the window itself.
		workspace.windowActivated.connect(function (w) {
		    if (w && isEditor(w)) bring(w);
		});
		// Refused: the same window, marked as demanding attention.
		var wins = workspace.windowList();
		for (var i = 0; i < wins.length; i++) {
		    if (!isEditor(wins[i])) continue;
		    (function (w) {
		        w.demandsAttentionChanged.connect(function () {
		            if (w.demandsAttention) bring(w);
		        });
		    })(wins[i]);
		}
		// No window held the project, so the editor opened one.
		workspace.windowAdded.connect(function (w) {
		    if (isEditor(w)) bring(w);
		});

		var fallback = new QTimer();
		fallback.singleShot = true;
		fallback.timeout.connect(function () {
		    if (done) return;
		    // An editor window in front already is the one the user is in, or
		    // the one the editor picked without a signal reaching us.
		    var active = workspace.activeWindow;
		    if (active && isEditor(active)) return;
		    var all = workspace.windowList();
		    for (var c = 0; c < wanted.length; c++) {
		        var hits = [];
		        for (var j = 0; j < all.length; j++) {
		            if (!isEditor(all[j])) continue;
		            if (String(all[j].caption).split("—")[0].trim() === wanted[c]) hits.push(all[j]);
		        }
		        if (hits.length === 1) bring(hits[0]);
		        if (hits.length > 0) return;
		    }
		});
		fallback.start(${fallback_ms});
	EOF

	busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting \
		loadScript ss "${watcher_js}" "${watcher_name}" >/dev/null 2>&1
	busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting \
		start >/dev/null 2>&1
	# KWin reads the file on a worker thread. The editor must not answer the
	# link before the script is listening, or its window goes unseen.
	sleep 0.5
}

stop_watcher() {
	[[ -n "${watcher_js}" ]] || return 0
	busctl --user call org.kde.KWin /Scripting org.kde.kwin.Scripting \
		unloadScript s "${watcher_name}" >/dev/null 2>&1
	rm -f "${watcher_js}"
}

start_watcher

# The directory rides in a URL query, so it is percent-encoded: a bare `&`
# would end the parameter and a `+` would decode as a space.
link="zed://agent?session=${session}"
if [[ -n "${cwd}" ]]; then
	cwd_query="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe="/"))' "${cwd}" 2>/dev/null)" &&
		link="${link}&cwd=${cwd_query}"
fi

# The editor's own CLI when one is installed, rather than xdg-open, and always
# with zed://. Zeo registers zeo:// with the desktop, so xdg-open zed:// finds
# nobody once Zeo replaces Zed -- and Zeo's own handler matches agent links
# only in their zed:// spelling, so zeo://agent would open nothing either.
# Both CLIs accept zed://, which is what makes one link work in both editors.
# Package CLIs, in order: zeo (app-editors/zeo, zeo-bin), zedit (zed),
# zedit-bin (zed-bin); only one of those packages installs at a time.
# ZEO_SYSTRAY_KDE_LINK_OPENER names a different program outright.
opener="${ZEO_SYSTRAY_KDE_LINK_OPENER:-}"
if [[ -z "${opener}" ]]; then
	opener="xdg-open"
	for cli in zeo zedit zedit-bin; do
		if command -v "${cli}" >/dev/null 2>&1; then
			opener="${cli}"
			break
		fi
	done
fi
"${opener}" "${link}" >/dev/null 2>&1

sleep "${watch_seconds}"
stop_watcher

exit 0
