# Set up xdg-desktop-portal and xdg-desktop-portal-wlr

This document describes how you can set up xdg-desktop-portal and xdg-desktop-portal-wlr for, e.g.
screencast.

## Install dependencies

Fedora:

```
$ dnf install xdg-desktop-portal-wlr xdg-desktop-portal pipewire wireplumber
```

## Check with non persistent way

1. Write `~/.config/xdg-desktop-portal/portals.conf`:

```
[preferred]
default=gnome-keyring;wlr;gtk
# or
# default=wlr
```

2. Write `monitor-related-components-for-screencast.sh`:

```
#!/bin/bash

set -x

if [ ! -z "$TMUX" ]; then
    echo "Unable to execute this script under a tmux session."
    exit 1
fi

SESSION_NAME=monitor-related-components-for-screencast

# Kill the session if it already exists to ensure a clean start.
tmux kill-session -t $SESSION_NAME 2>/dev/null

tmux new-session -d -s $SESSION_NAME \
     sh -c 'dbus-monitor --session | sed "s/Screen[A-Za-z]*/\x1B[31m&\x1B[0m/g"' \; \
     new-window tail -F /tmp/sabiniwm.log \; \
     new-window /usr/libexec/xdg-desktop-portal-wlr --replace -l DEBUG \; \
     new-window /usr/libexec/xdg-desktop-portal -r -v \; \
     new-window sh -c 'journalctl --user -fu pipewire | grep xdg'

tmux attach-session -t $SESSION_NAME
```

3. Update environment variables for daemons and restart them:

```
$ systemctl --user set-environment WAYLAND_DISPLAY="$WAYLAND_DISPLAY" XDG_CURRENT_DESKTOP= PIPEWIRE_DEBUG=D; systemctl --user show-environment; systemctl --user stop pipewire; systemctl --user restart wireplumber xdg-desktop-portal xdg-desktop-portal-wlr; cat "/proc/$(pidof xdg-desktop-portal)/environ" | tr \\0 \\n | pcregrep '^XDG_CURRENT_DESKTOP|WAYLAND_DISPLAY'
```

4. Run `bash monitor-related-components-for-screencast.sh`.

5. Run `XDG_SESSION_TYPE=wayland firefox`

6. Open <https://mozilla.github.io/webrtc-landing/gum_test.html> and start "Screen capture".

Then, screencast should work. If it don't work, see
<https://github.com/emersion/xdg-desktop-portal-wlr/wiki/%22It-doesn't-work%22-Troubleshooting-Checklist>
and investigate.

## Persist it

Run

```
$ systemctl --user set-environment WAYLAND_DISPLAY="$WAYLAND_DISPLAY"
```

or something like that in startup of sabiniwm.
