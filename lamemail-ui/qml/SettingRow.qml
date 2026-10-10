import QtQuick
import Lamemail

Item {
    id: row
    property string title
    property string hint
    property string link
    property string note
    property bool divided: true
    default property alias control: slot.data

    width: parent ? parent.width : 0
    height: line.height + (noteText.visible ? noteText.implicitHeight + 16 : 0)

    Item {
        id: line
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: Math.max(68, labels.implicitHeight + 32, slot.height + 32)
    }

    Column {
        id: labels
        anchors.left: parent.left
        anchors.right: slot.left
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        anchors.verticalCenter: line.verticalCenter
        spacing: 3

        Text {
            width: parent.width
            text: row.title
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize + 1
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            visible: text.length > 0
            text: row.hint
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
            wrapMode: Text.Wrap
        }

        Text {
            visible: row.link.length > 0
            text: row.link.replace(/^https?:\/\//, "")
            color: Theme.accent
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1

            HoverHandler {
                cursorShape: Qt.PointingHandCursor
            }

            TapHandler {
                onTapped: Qt.openUrlExternally(row.link)
            }
        }
    }

    Item {
        id: slot
        anchors.right: parent.right
        anchors.rightMargin: 24
        anchors.verticalCenter: line.verticalCenter
        width: childrenRect.width
        height: childrenRect.height
    }

    Text {
        id: noteText
        anchors.top: line.bottom
        anchors.topMargin: -8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 24
        anchors.rightMargin: 24
        visible: text.length > 0
        text: row.note
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize - 1
        wrapMode: Text.Wrap
    }

    Rule {
        visible: row.divided
        inset: 24
    }
}
