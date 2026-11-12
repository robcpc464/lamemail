import QtQuick
import Lamemail

MouseArea {
    id: overlay
    property var suspicion: null
    signal deleteRequested()
    signal readRequested()

    component Answer: MouseArea {
        id: choice
        property string label
        property string icon
        property bool danger: false
        readonly property color ink: danger ? Theme.red : Theme.foregroundBright

        width: content.implicitWidth + 28
        height: 34
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        Rectangle {
            anchors.fill: parent
            radius: Theme.radius
            color: Qt.alpha(choice.ink, choice.containsMouse ? 0.22 : 0.1)
            Behavior on color { ColorAnimation { duration: 120 } }
        }

        Row {
            id: content
            anchors.centerIn: parent
            spacing: 8

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: choice.icon
                size: 14
                color: choice.ink
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: choice.label
                color: choice.ink
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSize
            }
        }
    }

    visible: suspicion !== null
    opacity: visible ? 1 : 0
    hoverEnabled: true
    acceptedButtons: Qt.AllButtons
    onWheel: (wheel) => wheel.accepted = true
    Behavior on opacity { NumberAnimation { duration: 150 } }

    Rectangle {
        anchors.fill: parent
        color: Theme.background
        opacity: 0.8
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - 56, 520)
        height: column.height + 48
        radius: Theme.radius
        color: Theme.surface
        border.width: 1
        border.color: Qt.alpha(Theme.red, 0.6)

        Shadow {}

        Column {
            id: column
            x: 24
            y: 24
            width: parent.width - 48
            spacing: 14

            Row {
                spacing: 10

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "junk"
                    size: 18
                    color: Theme.red
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: ({
                        spam: Strings.suspectedSpam,
                        phishing: Strings.suspectedPhishing,
                        fraud: Strings.suspectedFraud,
                        malware: Strings.suspectedMalware
                    })[overlay.suspicion?.kind] ?? Strings.suspectedOther
                    color: Theme.foregroundBright
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize + 2
                    font.bold: true
                }
            }

            Column {
                width: parent.width
                visible: reason.text.length > 0
                spacing: 4

                Text {
                    text: Strings.aiAssessment
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize - 1
                }

                Text {
                    id: reason
                    width: parent.width
                    text: overlay.suspicion?.reason ?? ""
                    textFormat: Text.PlainText
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize + 1
                    wrapMode: Text.Wrap
                    elide: Text.ElideRight
                    maximumLineCount: Math.max(2, Math.floor((overlay.height - 200) / lines.lineSpacing))

                    FontMetrics {
                        id: lines
                        font: reason.font
                    }
                }
            }

            Row {
                anchors.right: parent.right
                spacing: 10

                Answer {
                    label: Strings.suspicionDelete
                    icon: "trash"
                    danger: true
                    onClicked: overlay.deleteRequested()
                }

                Answer {
                    label: Strings.suspicionRead
                    icon: "mail"
                    onClicked: overlay.readRequested()
                }
            }
        }
    }
}
