import QtQuick
import Lamemail

View {
    id: section
    property string key
    property string icon
    property string title
    property string hint
    property string error
    default property alias content: body.data

    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    width: Math.min(parent.width, 960)
    transform: Translate {
        y: section.active ? 0 : 10
        Behavior on y { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
    }

    Column {
        id: heading
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.topMargin: 28
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        spacing: 6

        Text {
            width: parent.width
            text: section.title
            color: Theme.foregroundBright
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 5
            font.bold: true
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            text: section.error.length > 0 ? section.error : section.hint
            color: section.error.length > 0 ? Theme.red : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
            wrapMode: Text.Wrap
        }
    }

    Item {
        id: body
        anchors.top: heading.bottom
        anchors.topMargin: 18
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
    }
}
