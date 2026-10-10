import QtQuick
import Lamemail

Rectangle {
    id: bar
    property var syncing: []
    property bool syncBusy: false
    property var progress: ({})
    property int sending: 0
    property string sendFailure
    property int pending: 0
    property bool pushing: false
    property var failing: []
    property string failures
    property var lastSync: null
    property int unread: 0

    height: 28
    color: Theme.backgroundDeep

    function progressText() {
        const progress = bar.progress
        const counted = progress.of > 0 ? progress.folder + " " + progress.fetched + "/" + progress.of : progress.folder
        const place = Strings.syncFolder.replace("{}", progress.folderNumber).replace("{}", progress.folders)
        return [progress.account, counted, place].join(" · ")
    }

    component Status: Row {
        id: status
        property bool active: false
        property int delay: 400
        property bool held: false
        property string icon
        property string text
        property color tint: Theme.muted
        property bool spinning: false
        property string tip

        visible: active && (delay === 0 || held)
        spacing: 6
        onActiveChanged: if (!active) held = false

        Timer {
            interval: status.delay
            running: status.active && status.delay > 0
            onTriggered: status.held = true
        }

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: status.icon
            size: 13
            color: status.tint

            RotationAnimation on rotation {
                running: status.spinning && status.visible
                loops: Animation.Infinite
                from: 0
                to: 360
                duration: 1400
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: text.length > 0
            text: status.text
            color: status.tint
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }

        HoverHandler {
            id: hover
        }

        Tip {
            visible: hover.hovered && status.tip.length > 0
            text: status.tip
        }
    }

    Rule {
        anchors.top: parent.top
        anchors.bottom: undefined
        opacity: 0.5
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: 24
        anchors.verticalCenter: parent.verticalCenter
        spacing: 18

        Status {
            id: failed
            active: bar.failing.length > 0
            delay: 0
            icon: "junk"
            tint: Theme.red
            text: bar.failing.join(", ")
            tip: bar.failures
        }

        Status {
            id: syncStatus
            active: bar.syncBusy || bar.syncing.length > 0
            icon: "sync"
            spinning: true
            text: bar.progress.folder ? bar.progressText() : bar.syncing.join(", ")
        }

        Status {
            id: sendStatus
            active: bar.sending > 0
            icon: "outbox"
            tint: bar.sendFailure.length > 0 ? Theme.red : Theme.muted
            text: String(bar.sending)
            tip: bar.sendFailure
        }

        Status {
            id: journalStatus
            active: bar.pending > 0 || bar.pushing
            icon: "upload"
            tint: bar.pushing ? Theme.accent : Theme.muted
            text: bar.pending > 0 ? String(bar.pending) : ""
        }

        Status {
            active: bar.lastSync !== null && !failed.visible && !syncStatus.visible
                    && !sendStatus.visible && !journalStatus.visible
            delay: 0
            icon: "check"
            text: Strings.current
            tip: bar.lastSync ? Strings.lastSyncTip.replace("{}", Qt.formatTime(bar.lastSync, "HH:mm")) : ""
        }
    }

    Status {
        anchors.right: parent.right
        anchors.rightMargin: 24
        anchors.verticalCenter: parent.verticalCenter
        active: bar.unread > 0
        delay: 0
        icon: "mail"
        tint: Theme.accent
        text: String(bar.unread)
        tip: Strings.unread.replace("{}", bar.unread)
    }
}
