import QtQuick
import QtQuick.Controls.Basic
import Lamemail

ScrollBar {
    id: bar
    policy: ScrollBar.AsNeeded
    minimumSize: 0.04
    padding: 2

    contentItem: Rectangle {
        implicitWidth: 4
        implicitHeight: 4
        radius: 2
        color: Theme.foreground
        opacity: bar.active ? 0.35 : 0
        Behavior on opacity { NumberAnimation { duration: 500 } }
    }

    background: Item {}
}
