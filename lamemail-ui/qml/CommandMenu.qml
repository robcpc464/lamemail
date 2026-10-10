pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Rectangle {
    id: menu

    property var commands: []
    property int current: 0
    readonly property int inset: 10
    readonly property int iconSize: 14
    readonly property int rowHeight: 32
    signal chosen(var command)

    width: Math.max(300, ...commands.map(command => list.x * 2 + inset * 4 + iconSize + names.advanceWidth("/" + command.name)
                                                    + labels.advanceWidth(command.label)))
    height: list.y + list.height + 4
    radius: Theme.radius
    color: Theme.surface
    border.width: 1
    border.color: Qt.alpha(Theme.line, 0.4)

    Shadow {}

    FontMetrics {
        id: names
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize
    }

    FontMetrics {
        id: labels
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 1
    }

    Column {
        id: list
        x: 4
        y: 4
        width: menu.width - 8

        Repeater {
            model: menu.commands

            Rectangle {
                id: row
                required property int index
                required property var modelData
                readonly property bool usable: modelData.enabled

                width: list.width
                height: menu.rowHeight
                radius: Theme.radius - 2
                color: row.index === menu.current ? Qt.alpha(Theme.accent, 0.16) : "transparent"

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: menu.inset
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: menu.inset

                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.modelData.icon
                        size: menu.iconSize
                        color: row.index === menu.current ? Theme.accent : Qt.alpha(Theme.muted, row.usable ? 1 : 0.6)
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "/" + row.modelData.name
                        color: row.usable ? Theme.foregroundBright : Theme.muted
                        font: names.font
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.modelData.label
                        color: Qt.alpha(Theme.muted, row.usable ? 1 : 0.6)
                        font: labels.font
                    }
                }
            }
        }
    }

    MouseArea {
        readonly property int row: Math.floor((mouseY - list.y) / menu.rowHeight)
        readonly property bool usable: row >= 0 && row < menu.commands.length && menu.commands[row].enabled

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: usable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onPositionChanged: if (usable) menu.current = row
        onClicked: if (usable) menu.chosen(menu.commands[row])
    }
}
