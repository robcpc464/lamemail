pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Item {
    id: item

    required property string kind
    required property string icon
    required property bool dense
    required property bool dim
    required property bool strike
    required property bool italic
    required property int indent
    required property string color
    required property bool iconSet
    required property bool denseSet
    required property bool dimSet
    required property bool strikeSet
    required property bool italicSet
    required property bool indentSet
    required property bool colorSet
    required property bool anySet
    required property int messages
    required property int unread
    required property bool exists
    property string picking

    width: ListView.view ? ListView.view.width : 0
    height: picking.length > 0 ? 96 : 48
    clip: true
    Behavior on height { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    function model() {
        return item.ListView.view.model
    }

    function pick(what) {
        item.picking = item.picking === what ? "" : what
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.foreground
        opacity: hover.hovered ? 0.06 : 0
        Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Item {
        id: line
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        height: 48

        Text {
            id: kindText
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, parent.width - controls.width - counts.width - 40)
            text: Styles.kindLabel(item.kind)
            color: item.exists ? Theme.foreground : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 1
            elide: Text.ElideRight
        }

        Text {
            id: counts
            anchors.left: kindText.right
            anchors.leftMargin: 14
            anchors.verticalCenter: parent.verticalCenter
            text: item.messages > 0
                  ? (item.unread > 0 ? item.messages + "  ·  " + Strings.unread.replace("{}", item.unread) : item.messages)
                  : ""
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }

        Row {
            id: controls
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                icon: item.icon
                tip: item.picking === "icon" ? "" : Styles.iconLabel(item.icon)
                on: item.picking === "icon"
                outlined: item.iconSet
                onClicked: item.pick("icon")
            }

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                swatch: Theme[item.color] ?? Theme.muted
                tip: item.picking === "color" ? "" : Strings.color
                on: item.picking === "color"
                outlined: item.colorSet
                onClicked: item.pick("color")
            }

            Repeater {
                model: Styles.flags

                Pill {
                    id: flag
                    required property string modelData
                    anchors.verticalCenter: parent.verticalCenter
                    text: Styles.flagLabel(modelData)
                    on: item[modelData]
                    outlined: item[modelData + "Set"]
                    onClicked: item.model().set(item.kind, flag.modelData, flag.on ? "false" : "true")
                }
            }

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                width: widestIndent.advanceWidth + 20
                text: Strings.indented + (item.indent > 0 ? " " + item.indent : "")
                on: item.indent > 0
                outlined: item.indentSet
                onClicked: item.model().set(item.kind, "indent", String((item.indent + 1) % (Styles.maxIndent + 1)))

                TextMetrics {
                    id: widestIndent
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 1
                    text: Strings.indented + " " + Styles.maxIndent
                }
            }

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                enabled: item.anySet
                opacity: enabled ? 1 : 0
                name: "reset"
                size: 16
                color: resetHover.hovered ? Theme.accent : Theme.muted

                HoverHandler {
                    id: resetHover
                    cursorShape: Qt.PointingHandCursor
                }

                TapHandler {
                    onTapped: item.model().reset(item.kind)
                }
            }
        }
    }

    Row {
        anchors.top: line.bottom
        anchors.right: parent.right
        anchors.rightMargin: 24
        spacing: 6
        visible: opacity > 0
        opacity: item.picking === "icon" ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Repeater {
            model: Styles.icons

            Pill {
                id: option
                required property string modelData
                text: Styles.iconLabel(modelData)
                icon: modelData
                on: modelData === item.icon
                onClicked: {
                    item.picking = ""
                    if (!option.on)
                        item.model().set(item.kind, "icon", option.modelData)
                }
            }
        }
    }

    Row {
        anchors.top: line.bottom
        anchors.right: parent.right
        anchors.rightMargin: 24
        spacing: 4
        visible: opacity > 0
        opacity: item.picking === "color" ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Repeater {
            model: Styles.colors

            Swatch {
                id: swatch
                required property string modelData
                colorName: modelData
                chosen: modelData === item.color
                onClicked: {
                    item.picking = ""
                    item.model().set(item.kind, "color", swatch.modelData)
                }
            }
        }
    }

    Rule {
        inset: 24
    }

    HoverHandler {
        id: hover
    }
}
