import QtQuick
import QtQuick.Controls.Basic
import Lamemail

Menu {
    id: menu

    required property var editor
    readonly property bool editable: !editor.readOnly
    readonly property bool selected: editor.selectedText.length > 0

    popupType: Popup.Item
    padding: 4

    background: Rectangle {
        implicitWidth: 220
        color: Theme.surface
        radius: Theme.radius
        border.width: 1
        border.color: Qt.alpha(Theme.line, 0.4)

        Shadow {}
    }

    TextMenuItem {
        key: "X"
        text: Strings.menuCut
        enabled: menu.editable && menu.selected
        onTriggered: menu.editor.cut()
    }

    TextMenuItem {
        key: "C"
        text: Strings.menuCopy
        enabled: menu.selected
        onTriggered: menu.editor.copy()
    }

    TextMenuItem {
        key: "V"
        text: Strings.menuPaste
        enabled: menu.editable && menu.editor.canPaste
        onTriggered: menu.editor.paste()
    }

    TextMenuItem {
        key: "A"
        text: Strings.menuSelectAll
        enabled: menu.editor.length > 0
        onTriggered: menu.editor.selectAll()
    }
}
