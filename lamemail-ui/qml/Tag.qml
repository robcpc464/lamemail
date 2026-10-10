import QtQuick
import Lamemail

Rectangle {
    id: tag
    property string text
    property color tint: Theme.accent

    width: label.implicitWidth + 12
    height: 16
    radius: Theme.radius - 2
    color: Qt.alpha(tint, 0.18)

    Text {
        id: label
        anchors.centerIn: parent
        text: tag.text.toUpperCase()
        color: tag.tint
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 2
        font.letterSpacing: 1
        font.bold: true
    }
}
