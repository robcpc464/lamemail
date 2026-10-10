import QtQuick
import Lamemail

Rectangle {
    required property Item control

    radius: Theme.radius
    color: Qt.alpha(Theme.foreground, 0.04)
    border.width: 1
    border.color: control.activeFocus ? Qt.alpha(Theme.accent, 0.7) : Qt.alpha(Theme.line, 0.35)
    Behavior on border.color { ColorAnimation { duration: 150 } }
}
