import QtQuick
import Lamemail

Item {
    id: toast
    property bool ok: true
    property string icon
    property var parts: []
    property string action
    property string actionKey
    signal actionTriggered()

    width: pill.width
    height: pill.height
    opacity: 0
    visible: opacity > 0
    Behavior on opacity { NumberAnimation { duration: 200 } }
    transform: Translate {
        y: toast.opacity > 0.5 ? 0 : 16
        Behavior on y { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
    }

    function show(message) {
        const good = message.ok ?? true
        const offer = message.action ?? ""
        if (good && offer.length === 0 && countdown.running)
            return
        label.text = message.text ?? ""
        ok = good
        icon = message.icon ?? ""
        parts = message.parts ?? []
        action = offer
        hide.interval = message.duration ?? 4000
        if (action.length > 0)
            countdown.restart()
        else
            countdown.stop()
        opacity = 1
        hide.restart()
    }

    function dismiss() {
        hide.stop()
        countdown.stop()
        opacity = 0
    }

    Timer {
        id: hide
        onTriggered: toast.opacity = 0
    }

    Rectangle {
        id: pill
        readonly property real room: (toast.parent ? toast.parent.width - 48 : 600) - 36
        width: content.implicitWidth + 36
        height: 38
        radius: Theme.radius
        color: Theme.surface
        border.width: 1
        border.color: toast.ok ? Qt.alpha(Theme.line, 0.4) : Theme.red

        Shadow {}

        MouseArea {
            anchors.fill: parent
        }

        Row {
            id: content
            anchors.centerIn: parent
            spacing: 12

            Icon {
                id: lead
                anchors.verticalCenter: parent.verticalCenter
                visible: toast.icon.length > 0
                name: toast.icon
                size: 16
                color: toast.ok ? Theme.accent : Theme.red
            }

            Row {
                id: counts
                anchors.verticalCenter: parent.verticalCenter
                visible: toast.parts.length > 0
                spacing: 14

                Repeater {
                    model: toast.parts

                    Row {
                        id: part
                        required property var modelData
                        spacing: 5

                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: part.modelData.icon
                            size: 14
                            color: Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: part.modelData.text
                            color: Theme.foreground
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontSize + 1
                        }
                    }
                }
            }

            Text {
                id: label
                readonly property real others: (lead.visible ? lead.width + content.spacing : 0)
                                                + (counts.visible ? counts.implicitWidth + content.spacing : 0)
                                                + (actions.visible ? actions.width + content.spacing : 0)
                anchors.verticalCenter: parent.verticalCenter
                visible: text.length > 0
                width: Math.min(implicitWidth, pill.room - others)
                color: Theme.foreground
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize + 1
                elide: Text.ElideRight
            }

            MouseArea {
                id: actions
                anchors.verticalCenter: parent.verticalCenter
                width: actionRow.implicitWidth
                height: actionRow.implicitHeight
                visible: toast.action.length > 0
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: toast.actionTriggered()

                Row {
                    id: actionRow
                    spacing: 8

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: toast.action
                        color: actions.containsMouse ? Theme.foregroundBright : Theme.accent
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize + 1
                        font.bold: true
                        Behavior on color { ColorAnimation { duration: 120 } }
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: toast.actionKey.length > 0
                        text: toast.actionKey
                        color: Theme.muted
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSize - 1
                    }
                }
            }
        }

        Rectangle {
            id: bar
            property real remaining: 0
            anchors.left: parent.left
            anchors.leftMargin: pill.radius
            anchors.bottom: parent.bottom
            width: (pill.width - 2 * pill.radius) * remaining
            height: 2
            visible: actions.visible
            color: Theme.accent
            opacity: 0.7
        }

        NumberAnimation {
            id: countdown
            target: bar
            property: "remaining"
            from: 1
            to: 0
            duration: hide.interval
        }
    }
}
