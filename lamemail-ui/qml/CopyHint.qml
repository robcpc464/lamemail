import QtQuick
import Lamemail

Rectangle {
    id: hint

    function show(x, y) {
        hint.x = x
        hint.y = y
        fade.restart()
    }

    width: row.implicitWidth + 18
    height: 24
    radius: Theme.radius
    color: Theme.surface
    border.width: 1
    border.color: Qt.alpha(Theme.accent, 0.5)
    opacity: 0
    visible: opacity > 0

    Shadow {}

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "check"
            size: 12
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Strings.copied
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }
    }

    SequentialAnimation {
        id: fade
        NumberAnimation { target: hint; property: "opacity"; to: 1; duration: 120 }
        PauseAnimation { duration: 900 }
        NumberAnimation { target: hint; property: "opacity"; to: 0; duration: 300 }
    }
}
