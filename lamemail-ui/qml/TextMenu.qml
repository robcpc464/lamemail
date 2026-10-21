import QtQuick
import QtQuick.Controls.Basic
import Lamemail

Menu {
    id: textMenu

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
        enabled: textMenu.editable && textMenu.selected
        onTriggered: textMenu.editor.cut()
    }

    TextMenuItem {
        key: "C"
        text: Strings.menuCopy
        enabled: textMenu.selected
        onTriggered: textMenu.editor.copy()
    }

    TextMenuItem {
        key: "V"
        text: Strings.menuPaste
        enabled: textMenu.editable && textMenu.editor.canPaste
        onTriggered: textMenu.editor.paste()
    }

    TextMenuItem {
        key: "A"
        text: Strings.menuSelectAll
        enabled: textMenu.editor.length > 0
        onTriggered: textMenu.editor.selectAll()
    }
}
