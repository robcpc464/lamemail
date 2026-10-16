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
    signal searchRequested()
    signal syncRequested()
    signal settingsRequested()
    signal composeRequested()

    Item {
        id: content
        anchors.centerIn: parent
        width: parent.width - 48
        height: parent.height

        Row {
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
