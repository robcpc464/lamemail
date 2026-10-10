import QtQuick
import QtQuick.Controls.Basic
import Lamemail

MenuItem {
    id: item

    property string key

    implicitHeight: 30
    leftPadding: 10
    rightPadding: 10
    opacity: enabled ? 1 : 0.4

    contentItem: Item {
        implicitWidth: label.implicitWidth + keyLabel.implicitWidth + 32

        Text {
            id: label
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: item.text
            color: item.highlighted ? Theme.foregroundBright : Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
        }

        Text {
            id: keyLabel
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: Strings.ctrlKey + "+" + item.key
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize - 1
        }
    }

    background: Rectangle {
        radius: Theme.radius - 2
        color: item.highlighted ? Qt.alpha(Theme.accent, 0.16) : "transparent"
    }
}
