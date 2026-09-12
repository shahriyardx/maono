import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.Ui
import qs.Commons

// Bar widget for maono: mute, gain, noise reduction and the RGB light for a
// Maono PD100W receiver, plus its battery level.
//
// Every read is one `maono status --json`, which answers in well under a
// second. Writes go through the matching subcommands, debounced and drained
// one process at a time so dragging the gain slider does not queue up a dozen
// HID round trips.
Panel {
  id: root
  moduleName: "maono"
  ipcTarget: "maono"

  // The bar sizes each widget from its root item, so without this the slot is
  // zero-width and the icon never draws.
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // Absolute path or bare name of the CLI. Override in shell.json when the
  // binary lives somewhere the shell's PATH does not cover.
  readonly property string binary: root.setting("binary", "maono")
  // Gain step per arrow press. The hardware range is 0-20.
  readonly property int step: root.setting("step", 1)

  property bool present: false
  property bool muted: false
  property int battery: 0
  property int gain: 0
  property int gainMax: 20
  property bool nrOn: false
  property int nrLevel: 0
  property bool lightOn: false
  property int lightMode: 0
  // Mode the user has clicked to but that has not reached the device yet.
  property int pendingMode: -1
  property bool loading: false
  property string error: ""

  // Gain the user has dragged to but that has not reached the device yet.
  // Cleared once the write completes, so the slider does not snap back.
  property int pendingGain: -1
  property bool gainQueued: false

  // Keyboard cursor. Rows: 0 gain, 1 noise reduction, 2 light, 3 colour
  // (the last one only reachable while the light is on). Mute is not a row -
  // it sits in the header and answers to m.
  property bool cursorActive: false
  property int selectedIndex: 0
  readonly property int rowCount: root.lightOn ? 4 : 3

  readonly property int lightModeMax: 8

  readonly property var nrNames: ["off", "low", "mid", "high"]

  // Light modes are colours, in the order the button on the mic cycles them.
  // The swatch colours are approximations for the chips, not device values.
  readonly property var lightModes: [
    { name: "white",      color: "#f2f2f2" },
    { name: "red",        color: "#e23b3b" },
    { name: "orange",     color: "#e98b26" },
    { name: "lime",       color: "#a6d629" },
    { name: "green",      color: "#33b054" },
    { name: "cyan",       color: "#2fc2bd" },
    { name: "blue",       color: "#3a6fe0" },
    { name: "purple",     color: "#9b5de5" },
    { name: "light blue", color: "#63c6f5" }
  ]

  function lightModeName(mode) {
    var m = root.lightModes[mode]
    return m === undefined ? String(mode) : m.name
  }

  readonly property int displayGain: pendingGain >= 0 ? pendingGain : gain
  readonly property int displayMode: pendingMode >= 0 ? pendingMode : lightMode

  readonly property string icon: root.muted ? "󰍭" : "󰍬"

  // 0 off, 1 low, 2 mid, 3 high. The device keeps the enable flag and the
  // level in separate fields; the widget shows them as one 4-step control.
  readonly property int nrStep: root.nrOn ? Math.min(3, root.nrLevel + 1) : 0

  onOpenedChanged: {
    if (opened) {
      cursorActive = false
      load()
    }
  }

  function load() {
    if (loadProc.running) return
    error = ""
    loading = true
    loadProc.command = [binary, "status", "--json"]
    loadProc.running = true
    watchdog.restart()
  }

  function apply(text) {
    var s = String(text || "").trim()
    if (s === "") {
      error = "No answer from maono"
      present = false
      return
    }
    var d
    try {
      d = JSON.parse(s)
    } catch (e) {
      error = "Could not read mic status"
      present = false
      return
    }
    muted = d.muted === true
    battery = d.battery === null ? 0 : d.battery
    gain = d.gain === null ? 0 : d.gain
    gainMax = d.gain_max === null ? 20 : d.gain_max
    nrOn = d.nr_on === true
    nrLevel = d.nr_level === null ? 0 : d.nr_level
    lightOn = d.light_on === true
    lightMode = d.light_mode === null ? 0 : d.light_mode
    present = true
    error = ""
  }

  function run(args) {
    if (writeProc.running) {
      // One HID conversation at a time. The reload after the current write
      // picks up whatever changed.
      return
    }
    writeProc.command = [binary].concat(args)
    writeProc.running = true
  }

  function setMute(on) {
    muted = on
    run([on ? "mute" : "unmute"])
  }

  function setLight(on) {
    lightOn = on
    run(["light", on ? "on" : "off"])
  }

  // `light <n>` switches the light on as well, which is what you want when
  // picking a mode from the panel.
  function setLightMode(value) {
    var v = Math.max(0, Math.min(lightModeMax, Math.round(value)))
    pendingMode = v
    lightOn = true
    run(["light", String(v)])
  }

  function nextLightMode() {
    setLightMode((displayMode + 1) % (lightModeMax + 1))
  }

  // Record locally and let the debounce collapse a drag into one write.
  function setGain(value) {
    pendingGain = Math.max(0, Math.min(gainMax, Math.round(value)))
    gainQueued = true
    debounce.restart()
  }

  function flushGain() {
    if (!gainQueued || writeProc.running || pendingGain < 0) return
    gainQueued = false
    run(["gain", String(pendingGain)])
  }

  function setNr(stepValue) {
    var v = Math.max(0, Math.min(3, Math.round(stepValue)))
    if (v === 0) {
      nrOn = false
      run(["nr", "off"])
    } else {
      nrOn = true
      nrLevel = v - 1
      run(["nr", nrNames[v]])
    }
  }

  function nudge(delta) {
    if (!present) return
    if (selectedIndex === 0) setGain(displayGain + delta * step)
    else if (selectedIndex === 1) setNr(nrStep + (delta > 0 ? 1 : -1))
    else if (selectedIndex === 2) setLight(delta > 0)
    else if (selectedIndex === 3) setLightMode(displayMode + delta)
  }

  Timer {
    id: debounce
    interval: 120
    onTriggered: root.flushGain()
  }

  // A binary that is not on PATH never starts, so onExited never fires and
  // the panel would sit on "Loading" for good. Say so instead.
  Timer {
    id: watchdog
    interval: 4000
    onTriggered: {
      if (!root.loading) return
      root.loading = false
      root.present = false
      if (root.error === "") {
        root.error = "Could not run " + root.binary
      }
    }
  }

  Process {
    id: loadProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.apply(text)
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var e = String(text || "").trim()
        if (e !== "") root.error = e
      }
    }
    onExited: function(exitCode) {
      watchdog.stop()
      root.loading = false
      if (exitCode !== 0) {
        root.present = false
        if (root.error === "") root.error = "Receiver not found"
      }
    }
  }

  Process {
    id: writeProc
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var e = String(text || "").trim()
        if (e !== "") root.error = e
      }
    }
    onExited: function(exitCode) {
      if (exitCode === 0) root.error = ""
      root.pendingGain = -1
      root.pendingMode = -1
      // Read back rather than trusting the optimistic local value: the
      // firmware clamps, and the physical buttons may have moved too.
      if (root.gainQueued) Qt.callLater(root.flushGain)
      else Qt.callLater(root.load)
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.icon

    onPressed: function(b) {
      if (root.opened) root.close()
      else root.open()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent

      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) {
          root.cursorActive = true
          if (dy !== 0) return
        }
        if (dy !== 0) {
          var next = root.selectedIndex + (dy > 0 ? 1 : -1)
          root.selectedIndex = Math.max(0, Math.min(root.rowCount - 1, next))
        }
        if (dx !== 0) root.nudge(dx > 0 ? 1 : -1)
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      // h j k l and x belong to PanelKeyCatcher's own navigation and never
      // reach here, so the light is on b - not l.
      onTextKey: function(t) {
        if (t === "m" || t === "M") root.setMute(!root.muted)
        else if (t === "b" || t === "B") root.setLight(!root.lightOn)
        else if (t === "n" || t === "N") root.nextLightMode()
        else if (t === "r" || t === "R") root.load()
      }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // ---------- Header: title · battery ----------
        Item {
          width: parent.width
          implicitHeight: Math.max(title.implicitHeight, muteToggle.implicitHeight)

          Column {
            id: title
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Text {
              text: "Microphone"
              color: Color.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              font.bold: true
            }

            Text {
              text: {
                if (root.error !== "") return root.error
                if (root.loading) return "Loading"
                if (!root.present) return "Receiver not found"
                return "Battery " + root.battery + "% · " + (root.muted ? "muted" : "live")
              }
              color: root.error !== "" ? Color.urgent : Color.foreground
              opacity: root.error !== "" ? 1.0 : 0.55
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }
          }

          // Mute lives in the header: it is the one control people open the
          // panel for, and the subtitle already reports its state.
          ToggleSwitch {
            id: muteToggle
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: root.present
            checked: root.muted
            foreground: Color.foreground
            onToggled: root.setMute(!root.muted)

            PanelToolTip {
              visible: muteToggle.containsMouse
              text: root.muted ? "Unmute the mic" : "Mute the mic"
            }
          }
        }

        PanelSeparator { width: parent.width }

        // ---------- Gain ----------
        Column {
          width: parent.width
          visible: root.present
          spacing: Style.space(4)

          Item {
            width: parent.width
            implicitHeight: gainLabel.implicitHeight

            Text {
              id: gainLabel
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              text: "Gain"
              color: Color.foreground
              opacity: (root.cursorActive && root.selectedIndex === 0) ? 1.0 : 0.75
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
            }

            Text {
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              text: root.displayGain + " / " + root.gainMax
              color: Color.foreground
              opacity: 0.55
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
            }
          }

          PanelSlider {
            width: parent.width
            bar: root.bar
            minimum: 0
            maximum: root.gainMax
            step: root.step
            integer: true
            value: root.displayGain
            onMoved: function(v) { root.setGain(v) }
            onReleased: function(v) { root.setGain(v) }
          }
        }

        // ---------- Noise reduction ----------
        // Four discrete settings, so a row of chips rather than a slider:
        // every option is visible and one click away.
        Column {
          width: parent.width
          visible: root.present
          spacing: Style.space(6)

          Text {
            id: nrLabel
            text: "Noise reduction"
            color: Color.foreground
            opacity: (root.cursorActive && root.selectedIndex === 1) ? 1.0 : 0.75
            font.family: Style.font.family
            font.pixelSize: Style.font.bodySmall
          }

          // ButtonGroup is a plain Row, so its chips size to their text and
          // leave the panel half empty. A RowLayout with fillWidth spreads
          // them across the full width instead.
          RowLayout {
            width: parent.width
            spacing: Style.spacing.md

            Repeater {
              model: root.nrNames

              delegate: Button {
                required property var modelData
                required property int index
                Layout.fillWidth: true
                text: modelData
                fontSize: Style.font.bodySmall
                bordered: true
                selected: index === root.nrStep
                hasCursor: root.cursorActive && root.selectedIndex === 1
                  && index === root.nrStep
                onClicked: {
                  root.cursorActive = true
                  root.selectedIndex = 1
                  root.setNr(index)
                }
              }
            }
          }
        }

        // ---------- RGB light, with its colours right underneath ----------
        // One block, tight spacing: the swatches belong to the switch above
        // them, not to a section of their own.
        Column {
          width: parent.width
          visible: root.present
          spacing: Style.space(6)

          Item {
            width: parent.width
            implicitHeight: Math.max(lightLabel.implicitHeight, lightToggle.implicitHeight)

            Text {
              id: lightLabel
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              text: "RGB light"
              color: Color.foreground
              opacity: (root.cursorActive && root.selectedIndex === 2) ? 1.0 : 0.75
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
            }

            ToggleSwitch {
              id: lightToggle
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              checked: root.lightOn
              foreground: Color.foreground
              onToggled: root.setLight(!root.lightOn)

              PanelToolTip {
                visible: lightToggle.containsMouse
                text: "Ring light on the mic"
              }
            }
          }

          // Nine colours. Three to a row divides evenly; four would leave the
          // last one alone. Each chip carries its own colour as the accent, so
          // the selected state and the border paint in that colour.
          Grid {
            id: modeGrid
            width: parent.width
            visible: root.lightOn
            columns: 3
            columnSpacing: Style.spacing.md
            rowSpacing: Style.spacing.md

            Repeater {
              model: root.lightModes

              delegate: Button {
                required property var modelData
                required property int index
                width: (modeGrid.width - modeGrid.columnSpacing * (modeGrid.columns - 1))
                  / modeGrid.columns
                text: modelData.name
                tooltipText: "Mode " + index
                fontSize: Style.font.caption
                bordered: true
                accent: modelData.color
                selected: index === root.displayMode
                hasCursor: root.cursorActive && root.selectedIndex === 3
                  && index === root.displayMode
                onClicked: {
                  root.cursorActive = true
                  root.selectedIndex = 3
                  root.setLightMode(index)
                }
              }
            }
          }
        }

        // ---------- Footer ----------
        PanelSeparator {
          width: parent.width
          visible: root.present
        }

        Text {
          width: parent.width
          visible: root.present
          text: "m mute · b light · n colour · r reload"
          color: Color.foreground
          opacity: 0.4
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
