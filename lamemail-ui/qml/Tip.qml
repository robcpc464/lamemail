import QtQuick
import QtQuick.Controls.Basic
import Lamemail

ToolTip {
    id: tip
    property alias wrapMode: label.wrapMode

    delay: 200

    contentItem: Text {
        id: label
        text: tip.text
        color: Theme.foreground
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontSize
    }

    background: Rectangle {
        color: Theme.surface
        radius: Theme.radius
        border.width: 1
        border.color: Qt.alpha(Theme.line, 0.4)

        Shadow {}
    }
}
