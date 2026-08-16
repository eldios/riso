// The riso theme carousel: every theme riso can see, as a cover-flow strip
// of preview cards. Left/Right browse, Enter applies, Escape leaves.
//
// A standalone Quickshell window on the Wayland overlay layer, so it works
// the same whichever desktop shell owns the screen.

import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import QtQuick.Effects

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
    id: win
    visible: true
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "riso-carousel"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

    // A scrim that breathes: darkest behind the strip, lighter at the edges.
    Rectangle {
      anchors.fill: parent
      gradient: Gradient {
        GradientStop { position: 0.0; color: "#a6000000" }
        GradientStop { position: 0.5; color: "#e0000000" }
        GradientStop { position: 1.0; color: "#a6000000" }
      }
      MouseArea { anchors.fill: parent; onClicked: Qt.quit() }
    }

    Column {
      anchors.centerIn: parent
      spacing: 26
      width: parent.width

      ListView {
        id: strip
        width: parent.width
        height: 480
        orientation: ListView.Horizontal
        spacing: -46
        model: root.themes
        currentIndex: root.current
        highlightMoveDuration: 180
        preferredHighlightBegin: width / 2 - 170
        preferredHighlightEnd: width / 2 + 170
        highlightRangeMode: ListView.StrictlyEnforceRange
        maximumFlickVelocity: 4200
        focus: true

        onCurrentIndexChanged: root.current = currentIndex

        Keys.onLeftPressed: decrementCurrentIndex()
        Keys.onRightPressed: incrementCurrentIndex()
        Keys.onReturnPressed: root.apply()
        Keys.onEnterPressed: root.apply()
        Keys.onEscapePressed: Qt.quit()

        delegate: Item {
          id: slot
          width: 340
          height: 480
          z: isCurrent ? 100 : 50 - Math.abs(index - strip.currentIndex)

          property bool isCurrent: ListView.isCurrentItem
          property int side: index === strip.currentIndex
                             ? 0 : (index < strip.currentIndex ? -1 : 1)

          Item {
            anchors.centerIn: parent
            width: 320
            height: slot.isCurrent ? 460 : 380
            Behavior on height { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

            transform: Rotation {
              origin.x: 160
              origin.y: 220
              axis { x: 0; y: 1; z: 0 }
              angle: slot.side * 32
              Behavior on angle { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
            }

            opacity: slot.isCurrent ? 1.0 : 0.55
            Behavior on opacity { NumberAnimation { duration: 150 } }

            Rectangle {
              id: card
              anchors.fill: parent
              radius: 16
              color: "#181a1f"
              border.width: slot.isCurrent ? 2 : 1
              border.color: slot.isCurrent ? "#9fbcd8" : "#33ffffff"
              clip: true

              Image {
                id: shot
                anchors.fill: parent
                anchors.margins: 3
                source: modelData.preview ? "file://" + modelData.preview : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                visible: modelData.preview !== ""
              }

              // Legible name on any image: fade the bottom, then write on it.
              Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 96
                gradient: Gradient {
                  GradientStop { position: 0.0; color: "#00000000" }
                  GradientStop { position: 1.0; color: "#d9000000" }
                }
              }

              Column {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 18
                spacing: 6

                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  text: modelData.name
                  color: "#f2f4f6"
                  font.pixelSize: slot.isCurrent ? 21 : 17
                  font.bold: slot.isCurrent
                  style: Text.Raised
                  styleColor: "#80000000"
                }

                Row {
                  anchors.horizontalCenter: parent.horizontalCenter
                  spacing: 6
                  visible: modelData.name === root.activeTheme
                  Rectangle { width: 7; height: 7; radius: 3.5; color: "#9fbcd8"
                              anchors.verticalCenter: parent.verticalCenter }
                  Text { text: "current"; color: "#c9d4de"; font.pixelSize: 12 }
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

            layer.enabled: true
            layer.effect: MultiEffect {
              shadowEnabled: true
              shadowBlur: slot.isCurrent ? 1.0 : 0.5
              shadowColor: "#aa000000"
              shadowVerticalOffset: 10
            }
          }
        }
      }

      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 22

        Text { text: "← →  browse"; color: "#8b939c"; font.pixelSize: 13 }
        Text { text: "enter  apply"; color: "#8b939c"; font.pixelSize: 13 }
        Text { text: "esc  close"; color: "#8b939c"; font.pixelSize: 13 }
        Text {
          text: root.busy ? "applying…" : ""
          color: "#c9d4de"; font.pixelSize: 13
        }
      }
    }
  }
}
