import QtQuick
import QtQuick.Controls.Basic
import Lamemail

TextField {
    id: field

    implicitWidth: 220
    implicitHeight: 32
    leftPadding: 10
    rightPadding: 10
    topPadding: 6
    bottomPadding: 6
    color: Theme.foreground
    placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
    selectionColor: Qt.alpha(Theme.accent, 0.35)
    selectedTextColor: Theme.foregroundBright
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontSize
    ContextMenu.menu: TextMenu {
        editor: field
    }

    background: FieldFrame {
        control: field
    }
}
