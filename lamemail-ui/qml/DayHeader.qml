import QtQuick
import Lamemail

Rectangle {
    id: header
    required property string section

    width: parent ? parent.width : 0
    color: Theme.background

    Text {
        id: label
        anchors.left: parent.left
        anchors.leftMargin: 24
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 8
        text: header.section.toUpperCase()
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 2
        font.bold: true
        font.letterSpacing: 1.2
    }

    Rectangle {
        anchors.left: label.right
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 24
        anchors.verticalCenter: label.verticalCenter
        height: 1
        color: Theme.line
        opacity: 0.25
    }
}
