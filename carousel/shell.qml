// The riso theme carousel, drawn the way Omarchy draws its image picker: a
// centred panel where the selected slice expands in place into the large
// slanted preview, and its neighbours stay packed beside it as narrow
// slanted slivers. Left/Right browse, type to filter, Enter applies,
// Escape clears the filter and then leaves.
//
// A standalone Quickshell window on the Wayland overlay layer, so it works
// the same whichever desktop shell owns the screen.

import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick
import QtQuick.Shapes
import QtQuick.Effects

ShellRoot {
  id: root

  property var themes: []
  property int selectedIndex: 0
  property string activeTheme: ""
  property string filterText: ""
  property bool busy: false

  function matches(i) {
    if (!filterText) return true
    return themes[i].name.toLowerCase().includes(filterText.toLowerCase())
  }

  // Position of item i among the items the filter keeps.
  function filteredPosition(i) {
    let pos = 0
    for (let k = 0; k < i; k++) if (matches(k)) pos++
    return pos
  }

  function selectAdjacent(direction) {
    let i = selectedIndex
    for (let step = 0; step < themes.length; step++) {
      i += direction
      if (i < 0 || i >= themes.length) return
      if (matches(i)) { selectedIndex = i; return }
    }
  }

  function updateFilter(text) {
    filterText = text
    if (themes.length === 0) return
    if (!matches(selectedIndex)) {
      for (let i = 0; i < themes.length; i++) {
        if (matches(i)) { selectedIndex = i; return }
      }
    }
  }

  function apply() {
    if (busy || themes.length === 0 || !matches(selectedIndex)) return
    busy = true
    const applyCmd = Quickshell.env("RISO_CAROUSEL_APPLY") || "riso set"
    applyProc.command = ["sh", "-c", applyCmd + " \"$1\"", "riso-carousel",
                         themes[selectedIndex].value]
    applyProc.running = true
  }

  // name<TAB>preview per line, from the sibling script.
  // What to list, what is current, and what a selection runs all come from
  // the launcher: the same strip serves themes and backgrounds alike.
  Process {
    id: listProc
    running: true
    command: [Quickshell.env("RISO_CAROUSEL_LIST")
              || Qt.resolvedUrl("list-themes.sh").toString().replace("file://", "")]
    stdout: StdioCollector {
      onStreamFinished: {
        const rows = []
        for (const line of text.split("\n")) {
          if (!line.trim()) continue
          const cells = line.split("\t")
          rows.push({ name: cells[0], preview: cells[1] || "",
                      value: cells[2] || cells[0] })
        }
        root.themes = rows
        for (let i = 0; i < rows.length; i++) {
          if (rows[i].value === root.activeTheme) { root.selectedIndex = i; break }
        }
      }
    }
  }

  Process {
    id: currentProc
    running: true
    command: ["sh", "-c",
      (Quickshell.env("RISO_CAROUSEL_CURRENT")
       || "cat \"${XDG_STATE_HOME:-$HOME/.local/state}/riso/current/theme.name\"")
      + " 2>/dev/null"]
    stdout: StdioCollector {
      onStreamFinished: {
        root.activeTheme = text.trim()
        for (let i = 0; i < root.themes.length; i++) {
          if (root.themes[i].value === root.activeTheme) { root.selectedIndex = i; break }
        }
      }
    }
  }

  Process {
    id: applyProc
    onExited: Qt.quit()
  }

  PanelWindow {
    id: win
    visible: true
    anchors { top: true; bottom: true; left: true; right: true }
    color: "transparent"
    WlrLayershell.namespace: "riso-carousel"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

    // Omarchy's picker geometry, scaled to the monitor instead of fixed:
    // their 768-wide preview with 108-wide slices overlapping by 30 and a
    // 28-point lean, all kept in the same proportions.
    property int expandedW: Math.round(width * 0.42)
    property int expandedH: Math.round(expandedW * 9 / 16)
    property int sliceW: Math.round(expandedW * 0.14)
    property int sliceH: Math.round(expandedH * 0.91)
    property int sliceSpacing: -Math.round(sliceW * 0.28)
    property real skew: sliceH * 0.065
    property real itemStep: sliceW + sliceSpacing

    Rectangle {
      anchors.fill: parent
      color: "#000000"
      opacity: 0.72
      MouseArea { anchors.fill: parent; onClicked: Qt.quit() }
    }

    Item {
      id: carousel
      anchors.centerIn: parent
      width: win.expandedW + 14 * win.itemStep
      height: win.expandedH
      clip: false
      focus: true

      readonly property real previewX: (width - win.expandedW) / 2

      Keys.priority: Keys.BeforeItem
      Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Escape) {
          if (root.filterText) root.updateFilter("")
          else Qt.quit()
          event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          root.apply()
          event.accepted = true
        } else if (event.key === Qt.Key_Left
                   || (event.key === Qt.Key_Tab && event.modifiers & Qt.ShiftModifier)
                   || event.key === Qt.Key_Backtab) {
          root.selectAdjacent(-1)
          event.accepted = true
        } else if (event.key === Qt.Key_Right || event.key === Qt.Key_Tab) {
          root.selectAdjacent(1)
          event.accepted = true
        } else if (event.key === Qt.Key_Backspace) {
          root.updateFilter(root.filterText.slice(0, -1))
          event.accepted = true
        } else if (event.text && event.text.length === 1
                   && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127
                   && (event.modifiers === Qt.NoModifier || event.modifiers === Qt.ShiftModifier)) {
          root.updateFilter(root.filterText + event.text)
          event.accepted = true
        }
      }

      Component.onCompleted: forceActiveFocus()

      Repeater {
        model: root.themes.length

        delegate: Item {
          id: item
          required property int index

          readonly property var themeData: root.themes[index]
          readonly property bool matched: root.matches(index)
          readonly property int relativeIndex: root.filteredPosition(index)
                                               - root.filteredPosition(root.selectedIndex)
          readonly property bool selected: matched && index === root.selectedIndex
          readonly property bool nearby: matched && Math.abs(relativeIndex) <= 16

          visible: nearby
          x: selected ? carousel.previewX
             : (relativeIndex < 0
                ? carousel.previewX + relativeIndex * win.itemStep
                : carousel.previewX + win.expandedW + win.sliceSpacing
                  + (relativeIndex - 1) * win.itemStep)
          width: selected ? win.expandedW : win.sliceW
          height: selected ? win.expandedH : win.sliceH
          y: (win.expandedH - height) / 2
          z: selected ? 100 : 50 - Math.min(Math.abs(relativeIndex), 40)

          Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
          Behavior on width { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
          Behavior on height { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
          Behavior on y { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

          // Every slice, the open one included, is the same slanted
          // parallelogram: the top edge leads by the skew.
          readonly property real topLeft: win.skew
          readonly property real topRight: width
          readonly property real bottomRight: width - win.skew
          readonly property real bottomLeft: 0

          // The shadow is its own blurred polygon behind the slice, so the
          // image, the border and the shadow each keep their own geometry.
          Shape {
            anchors.fill: parent
            z: -1
            antialiasing: true
            preferredRendererType: Shape.CurveRenderer
            layer.enabled: true
            layer.smooth: true
            layer.effect: MultiEffect {
              blurEnabled: true
              blur: 0.5
              blurMax: 16
            }
            transform: Translate { y: 5 }
            ShapePath {
              fillColor: "#38000000"
              strokeColor: "transparent"
              startX: item.topLeft; startY: 0
              PathLine { x: item.topRight; y: 0 }
              PathLine { x: item.bottomRight; y: item.height }
              PathLine { x: item.bottomLeft; y: item.height }
              PathLine { x: item.topLeft; y: 0 }
            }
          }

          Item {
            id: maskShape
            anchors.fill: parent
            visible: false
            layer.enabled: true
            layer.samples: 4
            layer.smooth: true

            Shape {
              anchors.fill: parent
              antialiasing: true
              preferredRendererType: Shape.CurveRenderer
              ShapePath {
                fillColor: "white"
                strokeColor: "transparent"
                startX: item.topLeft; startY: 0
                PathLine { x: item.topRight; y: 0 }
                PathLine { x: item.bottomRight; y: item.height }
                PathLine { x: item.bottomLeft; y: item.height }
                PathLine { x: item.topLeft; y: 0 }
              }
            }
          }

          Item {
            anchors.fill: parent
            layer.enabled: true
            layer.smooth: true
            layer.samples: 4
            // Mask alone, nothing else in this effect: a shadow here would
            // pad the render target and slide the mask off the polygon the
            // border is drawn on.
            layer.effect: MultiEffect {
              maskEnabled: true
              maskSource: maskShape
              maskThresholdMin: 0.5
              maskSpreadAtMin: 0.18
            }

            Image {
              anchors.fill: parent
              source: item.themeData.preview ? "file://" + item.themeData.preview : ""
              fillMode: Image.PreserveAspectCrop
              asynchronous: true
              cache: true
              smooth: true
            }

            Rectangle {
              anchors.fill: parent
              color: "#101318"
              opacity: item.selected ? 0 : 0.42
              Behavior on opacity { NumberAnimation { duration: 150 } }
            }
          }

          // A hairline of silver drawn on the same polygon: it cleans the
          // slanted cut to the eye and frames every slice alike.
          Shape {
            anchors.fill: parent
            antialiasing: true
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
              fillColor: "transparent"
              strokeColor: item.selected ? "#dfe3e7" : "#b5cfd4d9"
              // Centred on the cut: thick enough to cover the whole mask
              // transition band on both sides.
              strokeWidth: item.selected ? 4 : 2.5
              startX: item.topLeft; startY: 0
              PathLine { x: item.topRight; y: 0 }
              PathLine { x: item.bottomRight; y: item.height }
              PathLine { x: item.bottomLeft; y: item.height }
              PathLine { x: item.topLeft; y: 0 }
            }
          }

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: item.selected ? root.apply() : (root.selectedIndex = index)
          }
        }
      }
    }

    // Name under the carousel, the way their labels sit under the strip.
    Column {
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.top: carousel.bottom
      anchors.topMargin: 24
      spacing: 10

      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 10

        Text {
          text: root.themes.length > 0 && root.matches(root.selectedIndex)
                ? root.themes[root.selectedIndex].name : ""
          color: "#f2f4f6"
          font.pixelSize: 24
          font.bold: true
        }
        Text {
          visible: root.themes.length > 0
                   && root.themes[root.selectedIndex].value === root.activeTheme
          anchors.verticalCenter: parent.verticalCenter
          text: "● current"
          color: "#9fbcd8"
          font.pixelSize: 13
        }
      }

      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        visible: root.filterText !== ""
        text: "filter: " + root.filterText
        color: "#c9d4de"
        font.pixelSize: 14
      }

      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 26

        Text { text: "← →  browse"; color: "#8b939c"; font.pixelSize: 13 }
        Text { text: "type  filter"; color: "#8b939c"; font.pixelSize: 13 }
        Text { text: "enter  apply"; color: "#8b939c"; font.pixelSize: 13 }
        Text { text: root.busy ? "applying…" : "esc  close"; color: "#8b939c"; font.pixelSize: 13 }
      }
    }
  }
}
