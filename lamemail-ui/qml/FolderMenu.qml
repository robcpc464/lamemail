pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import Lamemail

Popup {
    id: menu

    property var tree: []
    property string current
    property real room: 420
    readonly property int rowHeight: 32
    readonly property int indent: 16
    readonly property string query: search.text.trim().toLowerCase()
    readonly property var rows: query.length === 0
        ? tree
        : tree.filter(row => row.selectable && row.name.toLowerCase().includes(query))
              .map(row => Object.assign({}, row, { label: row.name, depth: 0 }))
    signal chosen(string path)

    function usable(row) {
        return row.selectable && row.path !== menu.current
    }

    function step(from, by) {
        for (let at = from + by; at >= 0 && at < menu.rows.length; at += by) {
            if (menu.usable(menu.rows[at])) {
                list.currentIndex = at
                list.positionViewAtIndex(at, ListView.Contain)
                return
            }
        }
    }

    function pick(at) {
        const row = menu.rows[at]
        if (!row || !menu.usable(row))
            return
        menu.close()
        menu.chosen(row.path)
    }

    width: 340
    height: Math.min(topPadding + bottomPadding + search.height + 4 + Math.max(list.contentHeight, rowHeight), room)
    margins: 8
    padding: 4
    focus: true
    onRowsChanged: Qt.callLater(menu.step, -1, 1)
    onAboutToShow: {
        search.clear()
        menu.room = Math.max(160, menu.parent.mapToItem(null, 0, 0).y - 16)
    }
    onOpened: search.forceActiveFocus()

    background: Rectangle {
        radius: Theme.radius
        color: Theme.surface
        border.width: 1
        border.color: Qt.alpha(Theme.line, 0.4)

        Shadow {}
    }

    contentItem: Item {
        SearchInput {
            id: search
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            placeholderText: Strings.searchFolders
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Down)
                    menu.step(list.currentIndex, 1)
                else if (event.key === Qt.Key_Up)
                    menu.step(list.currentIndex, -1)
                else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                    menu.pick(list.currentIndex)
                else
                    return
                event.accepted = true
            }
        }

        ListView {
            id: list
            anchors.top: search.bottom
            anchors.topMargin: 4
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            clip: true
            model: menu.rows
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: SlimScrollBar {}

            delegate: Rectangle {
                id: row
                required property int index
                required property var modelData
                readonly property bool usable: menu.usable(modelData)
                readonly property bool here: modelData.selectable && modelData.path === menu.current

                width: list.width
                height: menu.rowHeight
                radius: Theme.radius - 2
                color: row.usable && row.index === list.currentIndex ? Qt.alpha(Theme.accent, 0.16) : "transparent"

                Icon {
                    id: kindIcon
                    x: 10 + row.modelData.depth * menu.indent
                    anchors.verticalCenter: parent.verticalCenter
                    name: row.modelData.kind.length > 0 && row.modelData.kind !== "other"
                          ? Styles.kindIcon(row.modelData.kind) : "folder"
                    size: 14
                    color: row.here ? Theme.accent : Qt.alpha(Theme.muted, row.usable ? 1 : 0.6)
                }

                Text {
                    anchors.left: kindIcon.right
                    anchors.leftMargin: 10
                    anchors.right: syncIcon.left
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.modelData.label
                    color: row.usable ? Theme.foregroundBright : Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontSize
                    elide: Text.ElideRight
                }

                Icon {
                    id: syncIcon
                    anchors.right: parent.right
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.modelData.sync
                    name: "sync"
                    size: 12
                    color: Theme.muted
                }

                MouseArea {
                    anchors.fill: parent
                    enabled: row.usable
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onEntered: list.currentIndex = row.index
                    onClicked: menu.pick(row.index)
                }
            }
        }

        WheelArea {
            flickable: list
        }

        Text {
            anchors.top: search.bottom
            anchors.topMargin: 12
            anchors.horizontalCenter: parent.horizontalCenter
            visible: menu.rows.length === 0 && menu.query.length > 0
            text: Strings.noFolder
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontSize
        }
    }
}
