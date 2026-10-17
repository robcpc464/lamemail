pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Item {
    id: row

    required property var folder
    required property bool current
    readonly property string kind: folder.kind
    readonly property bool other: kind === "other"
    property bool picking: false
    signal assignRequested(string kind, var sync, bool makeDefault)

    width: parent ? parent.width : 0
    height: line.height + (picking ? picker.height + 10 : 0)
    clip: true
    opacity: enabled ? 1 : 0.5
    Behavior on height { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    Item {
        id: line
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        height: 44

        Text {
            anchors.left: parent.left
            anchors.right: controls.left
            anchors.rightMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            text: row.folder.path
            color: row.other || !row.folder.sync ? Theme.muted : Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
            elide: Text.ElideRight
        }

        Row {
            id: controls
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Pill {
                icon: Styles.kindIcon(row.kind)
                text: Styles.kindLabel(row.kind)
                on: row.picking
                outlined: !row.other
                onClicked: row.picking = !row.picking
            }

            Pill {
                opacity: !row.other && row.folder.sync ? 1 : 0
                enabled: opacity > 0
                text: Strings.defaultFolder
                tip: Strings.defaultFolderTip
                on: row.current
                onClicked: if (!row.current) row.assignRequested(row.kind, undefined, true)
            }

            Pill {
                opacity: row.other ? 0 : 1
                enabled: opacity > 0
                text: Strings.folderSync
                tip: Strings.folderSyncTip
                on: row.folder.sync
                onClicked: row.assignRequested(row.kind, !row.folder.sync, false)
            }
        }
    }

    Flow {
        id: picker
        anchors.top: line.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 6
        opacity: row.picking ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }

        Repeater {
            model: row.picking ? Styles.kinds : []

            Pill {
                id: option
                required property string modelData
                icon: Styles.kindIcon(modelData)
                text: Styles.kindLabel(modelData)
                on: modelData === row.kind
                onClicked: {
                    row.picking = false
                    if (!option.on)
                        row.assignRequested(option.modelData, undefined, false)
                }
            }
        }
    }
}
