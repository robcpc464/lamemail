import QtQuick
import Lamemail

Rectangle {
    id: header
    property string title
    property string tag
    property color tint: Theme.accent
    default property alias actions: actionRow.data
    signal backRequested()

    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: 56
    color: Theme.backgroundDeep

    IconButton {
        id: backButton
        anchors.left: parent.left
        anchors.leftMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        name: "back"
        tip: Strings.backTip
        onClicked: header.backRequested()
    }

    Tag {
        id: tagItem
        anchors.left: backButton.right
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        visible: header.tag.length > 0
        text: header.tag
        tint: header.tint
    }

    Text {
        anchors.left: tagItem.visible ? tagItem.right : backButton.right
        anchors.leftMargin: tagItem.visible ? 12 : 10
        anchors.right: actionRow.left
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        text: header.title
        color: Theme.foregroundBright
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize + 5
        font.bold: true
        elide: Text.ElideRight
    }

    Row {
        id: actionRow
        anchors.right: parent.right
        anchors.rightMargin: 24
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
    }

    Rule {
        opacity: 0.5
    }
}
