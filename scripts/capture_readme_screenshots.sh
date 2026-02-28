#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="${APP_BUNDLE:-$ROOT_DIR/target/release/bundle/macos/Lekhani.app}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/docs/screenshots}"
PROJECT_FILE="${PROJECT_FILE:-$ROOT_DIR/samples/launch-test.lekhani}"
APP_NAME="Lekhani"
MODE_TAB_Y_OFFSET="${MODE_TAB_Y_OFFSET:-48}"
NARRATIVE_TAB_X_OFFSET="${NARRATIVE_TAB_X_OFFSET:-390}"
EDIT_TAB_X_OFFSET="${EDIT_TAB_X_OFFSET:-465}"
VISUAL_TAB_X_OFFSET="${VISUAL_TAB_X_OFFSET:-535}"

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "App bundle not found at $APP_BUNDLE" >&2
  echo "Run 'make build' first." >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

activate_app() {
  osascript <<'APPLESCRIPT' >/dev/null
tell application "Lekhani" to activate
delay 0.5
APPLESCRIPT
}

wait_for_window() {
  for _ in $(seq 1 40); do
    if osascript <<'APPLESCRIPT' >/dev/null 2>&1
tell application "System Events"
  tell process "Lekhani"
    if (count of windows) > 0 then
      return 0
    end if
  end tell
end tell
APPLESCRIPT
    then
      return 0
    fi
    sleep 0.5
  done

  echo "Timed out waiting for Lekhani window." >&2
  exit 1
}

window_bounds() {
  osascript <<'APPLESCRIPT'
tell application "System Events"
  tell process "Lekhani"
    set {x1, y1} to position of window 1
    set {w, h} to size of window 1
    return (x1 as string) & ":" & (y1 as string) & ":" & (w as string) & ":" & (h as string)
  end tell
end tell
APPLESCRIPT
}

click_mode() {
  local mode_name="$1"
  if osascript <<APPLESCRIPT >/dev/null 2>&1
tell application "System Events"
  tell process "$APP_NAME"
    click (first button of window 1 whose name is "$mode_name")
  end tell
end tell
APPLESCRIPT
  then
    sleep 0.8
    return 0
  fi

  local window_position
  window_position="$(osascript <<'APPLESCRIPT'
tell application "System Events"
  tell process "Lekhani"
    set {x1, y1} to position of window 1
    return (x1 as string) & ":" & (y1 as string)
  end tell
end tell
APPLESCRIPT
)"
  local x="${window_position%%:*}"
  local y="${window_position##*:}"
  local offset_x
  case "$mode_name" in
    Narrative) offset_x="$NARRATIVE_TAB_X_OFFSET" ;;
    Edit) offset_x="$EDIT_TAB_X_OFFSET" ;;
    Visual) offset_x="$VISUAL_TAB_X_OFFSET" ;;
    *)
      echo "Unknown mode '$mode_name'" >&2
      exit 1
      ;;
  esac
  local target_x=$((x + offset_x))
  local target_y=$((y + MODE_TAB_Y_OFFSET))

  osascript <<APPLESCRIPT >/dev/null
tell application "$APP_NAME" to activate
delay 0.2
tell application "System Events"
  click at {$target_x, $target_y}
end tell
APPLESCRIPT
  sleep 0.8
}

capture_window() {
  local filename="$1"
  local bounds
  bounds="$(window_bounds | tr -d '[:space:]')"
  local x="${bounds%%:*}"
  local remainder="${bounds#*:}"
  local y="${remainder%%:*}"
  remainder="${remainder#*:}"
  local width="${remainder%%:*}"
  local height="${remainder##*:}"
  screencapture -x -R"${x},${y},${width},${height}" "$OUTPUT_DIR/$filename"
}

pkill -f "/Lekhani.app" 2>/dev/null || true
sleep 1

if [[ -f "$PROJECT_FILE" ]]; then
  open -n "$APP_BUNDLE" --args "$PROJECT_FILE"
else
  open -n "$APP_BUNDLE"
fi

wait_for_window
activate_app

click_mode "Narrative"
capture_window "narrative-mode.png"

click_mode "Visual"
capture_window "visual-inspector.png"

echo "Saved screenshots to $OUTPUT_DIR"
