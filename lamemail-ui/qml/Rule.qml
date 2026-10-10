import QtQuick
import Lamemail

Rectangle {
    property int inset: 0

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    anchors.leftMargin: inset
    anchors.rightMargin: inset
    height: 1
    color: Theme.line
    opacity: 0.25
}
