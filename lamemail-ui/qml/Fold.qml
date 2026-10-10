import QtQuick

Item {
    id: fold
    property bool open: false
    default property alias content: column.data

    width: parent ? parent.width : 0
    height: open ? column.implicitHeight : 0
    clip: true
    opacity: open ? 1 : 0
    enabled: open
    Behavior on height { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    Behavior on opacity { NumberAnimation { duration: 150 } }

    Column {
        id: column
        width: parent.width
    }
}
