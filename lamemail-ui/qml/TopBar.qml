pragma ComponentBehavior: Bound

import QtQuick
import Lamemail

Rectangle {
    id: bar
    height: 56
    color: Theme.backgroundDeep

    property bool searching: false
    property bool syncing: false
    property bool filtering: true
    property string kind
    property int trash
    signal searchRequested()
    signal syncRequested()
    signal settingsRequested()
    signal composeRequested()
    signal emptyTrashRequested()

    Item {
        id: content
        anchors.centerIn: parent
        width: parent.width - 48
        height: parent.height

        Row {
            id: filters
            anchors.verticalCenter: parent.verticalCenter
            spacing: 14

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Strings.wordmark
                color: Theme.foregroundBright
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize + 6
                font.bold: true
            }

            Row {
                anchors.verticalCenter: parent.verticalCenter
                enabled: bar.filtering
                opacity: enabled ? 1 : 0.4
                Behavior on opacity { NumberAnimation { duration: 150 } }

                Repeater {
                    model: Styles.mailKinds

                    IconButton {
                        required property string modelData
                        readonly property bool chosen: bar.kind === modelData

                        name: Styles.kindIcon(modelData)
                        iconSize: 18
                        on: chosen
                        iconColor: chosen ? Theme.accent : Theme.muted
                        tip: Strings.kindOnly.replace("{}", Styles.kindLabel(modelData))
                        onClicked: bar.kind = chosen ? "" : modelData
                    }
                }
            }
        }

        Row {
            id: emptying
            anchors.left: filters.right
            anchors.leftMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            visible: bar.trash > 0
            spacing: 8

            readonly property string label: Strings.emptyTrash.replace("{}", bar.trash)
            readonly property bool roomy: tools.x - emptying.x - divider.width - emptying.spacing - 16
                                          >= labelSize.advanceWidth + 20

            TextMetrics {
                id: labelSize
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize - 1
                text: emptying.label
            }

            Divider {
                id: divider
            }

            Pill {
                anchors.verticalCenter: parent.verticalCenter
                text: emptying.roomy ? emptying.label : String(bar.trash)
                icon: emptying.roomy ? "" : "trash"
                tip: emptying.roomy ? "" : emptying.label
                danger: true
                onClicked: bar.emptyTrashRequested()
            }
        }

        Row {
            id: tools
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4

            IconButton {
                name: "search"
                tip: Strings.searchTip
                iconColor: bar.searching ? Theme.accent : Theme.muted
                onClicked: bar.searchRequested()
            }

            IconButton {
                name: "sync"
                tip: Strings.syncTip
                enabled: !bar.syncing
                spinning: bar.syncing
                onClicked: bar.syncRequested()
            }

            IconButton {
                name: "compose"
                tip: Strings.composeTip
                onClicked: bar.composeRequested()
            }

            IconButton {
                name: "settings"
                tip: Strings.settingsTip
                dot: Preferences.checkUpdates && Updates.newer
                onClicked: bar.settingsRequested()
            }
        }
    }

    Rule {
        opacity: 0.5
    }
}
