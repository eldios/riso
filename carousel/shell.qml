// The riso theme carousel: every theme riso can see, as a horizontal strip
// of preview cards. Left/Right browse, Enter applies, Escape leaves.
//
// A standalone Quickshell window on the Wayland overlay layer, so it works
// the same whichever desktop shell owns the screen.

import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick

ShellRoot {
  id: root

  property var themes: []
  property int current: 0
  property string activeTheme: ""
  property bool busy: false

  // name<TAB>preview per line, from the sibling script.
  Process {
    id: listProc
    running: true
    command: [Qt.resolvedUrl("list-themes.sh").toString().replace("file://", "")]
    stdout: StdioCollector {
      onStreamFinished: {
        const rows = []
        for (const line of text.split("\n")) {
          if (!line.trim()) continue
          const cells = line.split("\t")
          rows.push({ name: cells[0], preview: cells[1] || "" })
        }
        root.themes = rows
        for (let i = 0; i < rows.length; i++) {
          if (rows[i].name === root.activeTheme) { root.current = i; break }
        }
      }
    }
  }

  Process {
    id: currentProc
    running: true
    command: ["sh", "-c",
      "cat \"${XDG_STATE_HOME:-$HOME/.local/state}/riso/current/theme.name\" 2>/dev/null"]
    stdout: StdioCollector {
      onStreamFinished: {
        root.activeTheme = text.trim()
        for (let i = 0; i < root.themes.length; i++) {
          if (root.themes[i].name === root.activeTheme) { root.current = i; break }
        }
      }
    }
  }

  Process {
    id: applyProc
    onExited: Qt.quit()
  }

  function apply() {
    if (busy || themes.length === 0) return
    busy = true
    const applyCmd = Quickshell.env("RISO_CAROUSEL_APPLY") || "riso set"
    applyProc.command = ["sh", "-c", applyCmd + " \"$1\"", "riso-carousel",
                         themes[current].name]
    applyProc.running = true
  }

  PanelWindow {
    visible: true
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "riso-carousel"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

    Rectangle {
      anchors.fill: parent
      color: "#000000"
      opacity: 0.6
      MouseArea { anchors.fill: parent; onClicked: Qt.quit() }
    }

    Column {
      anchors.centerIn: parent
      spacing: 18
      width: parent.width

      ListView {
        id: strip
        width: parent.width
        height: 320
        orientation: ListView.Horizontal
        spacing: 24
        model: root.themes
        currentIndex: root.current
        highlightMoveDuration: 150
        preferredHighlightBegin: width / 2 - 130
        preferredHighlightEnd: width / 2 + 130
        highlightRangeMode: ListView.StrictlyEnforceRange
        focus: true

        onCurrentIndexChanged: root.current = currentIndex

        Keys.onLeftPressed: decrementCurrentIndex()
        Keys.onRightPressed: incrementCurrentIndex()
        Keys.onReturnPressed: root.apply()
        Keys.onEnterPressed: root.apply()
        Keys.onEscapePressed: Qt.quit()

        delegate: Item {
          width: 260
          height: 320
          property bool isCurrent: ListView.isCurrentItem

          Rectangle {
            id: card
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.verticalCenter: parent.verticalCenter
            width: 250
            height: isCurrent ? 310 : 260
            radius: 12
            color: "#16181c"
            border.width: isCurrent ? 3 : 1
            border.color: isCurrent ? "#7f9fbf" : "#4a5058"
            clip: true
            Behavior on height { NumberAnimation { duration: 120 } }

            Image {
              anchors.fill: parent
              anchors.margins: 4
              source: modelData.preview ? "file://" + modelData.preview : ""
              fillMode: Image.PreserveAspectCrop
              asynchronous: true
              visible: modelData.preview !== ""
            }

            Rectangle {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.bottom: parent.bottom
              height: 44
              color: "#cc16181c"

              Text {
                anchors.centerIn: parent
                text: modelData.name
                     + (modelData.name === root.activeTheme ? "  (current)" : "")
                color: "#e6e9ec"
                font.pixelSize: 16
                font.bold: isCurrent
              }
            }

            MouseArea {
              anchors.fill: parent
              onClicked: {
                if (root.current === index) root.apply()
                else strip.currentIndex = index
              }
            }
          }
        }
      }

      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: root.busy ? "applying..." : "enter apply    esc close"
        color: "#7b8189"
        font.pixelSize: 13
      }
    }
  }
}
