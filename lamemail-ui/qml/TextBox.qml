import QtQuick
import QtQuick.Controls.Basic
import Lamemail

TextArea {
    id: box

    implicitWidth: 320
    implicitHeight: Math.max(contentHeight + topPadding + bottomPadding, 80)
    leftPadding: 10
    rightPadding: 10
    topPadding: 6
    bottomPadding: 6
    wrapMode: TextArea.Wrap
    color: Theme.foreground
    placeholderTextColor: Qt.alpha(Theme.muted, 0.8)
    selectionColor: Qt.alpha(Theme.accent, 0.35)
    selectedTextColor: Theme.foregroundBright
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontSize
    ContextMenu.menu: TextMenu {
        editor: box
    }

    background: FieldFrame {
        control: box
    }
}
